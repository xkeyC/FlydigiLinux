//! 服务的核心状态：游戏检测、会话（震动绑定 + 官方 mod 进程）管理、后台任务、IPC 请求处理。
//! 行为对应 Space Station 的 AdapterTriggerRunner.CheckGame / ModHelper。

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use flydigi_core::gamelist::{self, Game, Route};
use flydigi_core::ipc::{ActiveGame, GameView, Request, Response, Status};
use flydigi_core::store::{self, State};
use flydigi_core::{paths, procscan};
use flydigi_proto::{ForceTrigger, GripBind, TriggerSide};

use crate::device::{DeviceCommand, DeviceHandle};
use crate::dsmode::{DsMode, DsOptions};

/// 游戏还在运行时，mod 进程意外退出最多重启这么多次。
const MAX_RESTARTS: u32 = 3;

pub struct Daemon {
    inner: Mutex<Inner>,
    pub device: DeviceHandle,
    pub ptrace: (bool, String),
    pub udp_listening: AtomicBool,
    pub shutdown: AtomicBool,
    ds_mode: Mutex<Option<DsMode>>,
}

struct Inner {
    state: State,
    games: Vec<Game>,
    sessions: HashMap<i64, Session>,
    jobs: BTreeMap<i64, String>,
    errors: HashMap<i64, String>,
    last_effects: VecDeque<String>,
}

struct Session {
    pid: u32,
    process_name: String,
    mod_proc: Option<Child>,
    mod_command: Option<String>,
    restarts: u32,
    vibration: bool,
}

impl Daemon {
    pub fn new(device: DeviceHandle, ptrace: (bool, String)) -> Arc<Self> {
        let games = gamelist::load_cached().unwrap_or_else(|e| {
            log::warn!("读取游戏列表缓存失败：{e:#}");
            Vec::new()
        });
        Arc::new(Self {
            inner: Mutex::new(Inner {
                state: State::load(),
                games,
                sessions: HashMap::new(),
                jobs: BTreeMap::new(),
                errors: HashMap::new(),
                last_effects: VecDeque::new(),
            }),
            device,
            ptrace,
            udp_listening: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
            ds_mode: Mutex::new(None),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn udp_port(&self) -> u16 {
        self.lock().state.settings.udp_port
    }

    pub fn has_game_list(&self) -> bool {
        !self.lock().games.is_empty()
    }

    pub fn note_effects(&self, effects: &[ForceTrigger]) {
        let mut inner = self.lock();
        for e in effects {
            // 格式「L|R 模式号 [参数]」，由 GUI 按界面语言显示
            let side = if e.side == TriggerSide::Left { 'L' } else { 'R' };
            if inner.last_effects.len() == 20 {
                inner.last_effects.pop_front();
            }
            inner.last_effects.push_back(format!("{side} {} {:?}", e.mode, e.params));
        }
    }

    // ------------------------------------------------------------ 游戏检测

    pub fn watch_loop(self: &Arc<Self>) {
        while !self.shutdown.load(Ordering::Relaxed) {
            self.poll();
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    fn poll(self: &Arc<Self>) {
        let enabled: Vec<Game> = {
            let inner = self.lock();
            inner
                .games
                .iter()
                .filter(|g| {
                    let st = inner.state.game(g.id);
                    st.enabled && g.route().supported() && (!g.route().needs_download() || st.installed.is_some())
                })
                .cloned()
                .collect()
        };
        let wanted: Vec<String> = enabled.iter().flat_map(Game::process_names).collect();
        let found = if wanted.is_empty() { HashMap::new() } else { procscan::scan(&wanted) };

        let mut inner = self.lock();
        // 已停用或已退出的游戏
        let ids: Vec<i64> = inner.sessions.keys().copied().collect();
        for id in ids {
            let still_running = enabled
                .iter()
                .find(|g| g.id == id)
                .is_some_and(|g| g.process_names().iter().any(|n| found.contains_key(n)));
            if !still_running {
                let session = inner.sessions.remove(&id).expect("会话存在");
                let game = inner.games.iter().find(|g| g.id == id).cloned();
                drop(inner);
                self.stop_session(game.as_ref(), session);
                inner = self.lock();
            }
        }
        let Inner { sessions, state, .. } = &mut *inner;
        for game in &enabled {
            let candidates: Vec<(String, &procscan::Candidate)> = game
                .process_names()
                .into_iter()
                .filter_map(|n| found.get(&n).map(|c| (n, c)))
                .flat_map(|(n, list)| list.iter().map(move |c| (n.clone(), c)))
                .collect();
            let Some((name, candidate)) = candidates.iter().find(|(_, c)| c.mapped).or(candidates.first()) else {
                continue;
            };
            if let Some(session) = sessions.get_mut(&game.id) {
                self.check_mod(game, session, state);
                continue;
            }
            // XGameMonitor 自己按进程名找内存，得等真正的游戏进程映射了 exe 再启动
            if game.route() == Route::Monitor && !candidate.mapped {
                continue;
            }
            let session = self.start_session(game, original_case(game, name), candidate.pid, state);
            sessions.insert(game.id, session);
        }
    }

    fn start_session(&self, game: &Game, process_name: String, pid: u32, state: &State) -> Session {
        log::info!("检测到游戏：{}（{}，pid {pid}）", game.display_name(), game.route().label());
        let vibration = game.applies_vibration();
        if vibration {
            for bind in vibration_binds(game, true) {
                self.device.send(DeviceCommand::Bind(bind));
            }
        }
        let mut session =
            Session { pid, process_name, mod_proc: None, mod_command: None, restarts: 0, vibration };
        if matches!(game.route(), Route::Monitor | Route::Telemetry | Route::ExternalExe) {
            self.launch_mod(game, &mut session, state);
        }
        session
    }

    fn check_mod(&self, game: &Game, session: &mut Session, state: &State) {
        let Some(child) = session.mod_proc.as_mut() else {
            return;
        };
        let Ok(Some(status)) = child.try_wait() else {
            return;
        };
        log::warn!("{} 的 mod 进程退出了：{status}", game.display_name());
        session.mod_proc = None;
        if session.restarts >= MAX_RESTARTS {
            log::error!("{} 的 mod 已重启 {MAX_RESTARTS} 次，不再重启", game.display_name());
            return;
        }
        session.restarts += 1;
        self.launch_mod(game, session, state);
    }

    fn launch_mod(&self, game: &Game, session: &mut Session, state: &State) {
        let Some(exe) = find_mod_exe(game) else {
            log::error!("{} 的 mod 程序 {} 不存在，请重新安装", game.display_name(), game.mod_name);
            return;
        };
        match spawn_mod(&exe, &session.process_name, &state.settings) {
            Ok((child, command)) => {
                log::info!("启动官方 mod：{command}（pid {}）", child.id());
                session.mod_proc = Some(child);
                session.mod_command = Some(command);
            }
            Err(e) => log::error!("启动 {} 失败：{e:#}", exe.display()),
        }
    }

    fn stop_session(&self, game: Option<&Game>, mut session: Session) {
        let name = game.map_or("?", |g| g.display_name());
        log::info!("游戏已退出：{name}");
        if let Some(child) = session.mod_proc.as_mut() {
            kill_group(child);
        }
        if session.vibration {
            if let Some(game) = game {
                for bind in vibration_binds(game, false) {
                    self.device.send(DeviceCommand::Bind(bind));
                }
            }
        }
        self.device.send(DeviceCommand::Reset);
    }

    pub fn stop_all(&self) {
        self.ds_mode.lock().unwrap().take();
        let sessions: Vec<(i64, Session)> = self.lock().sessions.drain().collect();
        for (id, session) in sessions {
            let game = self.lock().games.iter().find(|g| g.id == id).cloned();
            self.stop_session(game.as_ref(), session);
        }
        self.device.send(DeviceCommand::Reset);
    }

    // ------------------------------------------------------------ 后台任务

    fn spawn_job(self: &Arc<Self>, id: i64, label: String, job: impl FnOnce(&Arc<Self>) -> anyhow::Result<()> + Send + 'static) {
        {
            let mut inner = self.lock();
            if inner.jobs.contains_key(&id) {
                return;
            }
            inner.jobs.insert(id, label.clone());
            inner.errors.remove(&id);
        }
        let this = self.clone();
        std::thread::spawn(move || {
            let result = job(&this);
            let mut inner = this.lock();
            inner.jobs.remove(&id);
            if let Err(e) = result {
                log::error!("{label}失败：{e:#}");
                inner.errors.insert(id, format!("{e:#}"));
            }
        });
    }

    pub fn refresh_list(self: &Arc<Self>) {
        self.spawn_job(0, "刷新游戏列表".into(), |this| {
            let games = gamelist::fetch()?;
            log::info!("游戏列表已更新：{} 个游戏", games.len());
            this.lock().games = games;
            Ok(())
        });
    }

    fn install(self: &Arc<Self>, id: i64, enable_after: bool) {
        let Some(game) = self.lock().games.iter().find(|g| g.id == id).cloned() else {
            return;
        };
        self.spawn_job(id, format!("安装 {} 的 mod", game.display_name()), move |this| {
            let current = this.lock().state.game(id);
            if let Some(old) = &current.installed {
                store::uninstall(&game, old)?;
            }
            let installed = store::install(&game, &current)?;
            log::info!("已安装 {}（{} 个文件 -> {}）", game.display_name(), installed.files.len(), installed.target.display());
            let mut inner = this.lock();
            let st = inner.state.game_mut(id);
            st.installed = Some(installed);
            if enable_after {
                st.enabled = true;
            }
            inner.state.save()
        });
    }

    fn uninstall(self: &Arc<Self>, id: i64) {
        let Some(game) = self.lock().games.iter().find(|g| g.id == id).cloned() else {
            return;
        };
        self.spawn_job(id, format!("卸载 {} 的 mod", game.display_name()), move |this| {
            let installed = this.lock().state.game(id).installed;
            if let Some(installed) = installed {
                store::uninstall(&game, &installed)?;
            }
            let mut inner = this.lock();
            let st = inner.state.game_mut(id);
            st.installed = None;
            st.enabled = false;
            log::info!("已卸载 {}", game.display_name());
            inner.state.save()
        });
    }

    // ------------------------------------------------------------ IPC

    pub fn handle(self: &Arc<Self>, request: Request) -> Response {
        match request {
            Request::Status => Response::Status(self.status()),
            Request::Games => Response::Games { games: self.game_views() },
            Request::RefreshList => {
                self.refresh_list();
                Response::Ok
            }
            Request::Install { id } => {
                self.install(id, false);
                Response::Ok
            }
            Request::Uninstall { id } => {
                self.uninstall(id);
                Response::Ok
            }
            Request::SetEnabled { id, enabled } => {
                let Some(game) = self.lock().games.iter().find(|g| g.id == id).cloned() else {
                    return Response::Error { message: format!("没有 id 为 {id} 的游戏") };
                };
                if !game.route().supported() {
                    return Response::Error { message: "这个游戏只有 DualSense 模式，暂不支持".into() };
                }
                let installed = self.lock().state.game(id).installed.is_some();
                // 官方启用游戏时，没装 mod 就先下载安装
                if enabled && game.route().needs_download() && !installed {
                    self.install(id, true);
                    return Response::Ok;
                }
                let mut inner = self.lock();
                inner.state.game_mut(id).enabled = enabled;
                into_response(inner.state.save())
            }
            Request::SetGameDir { id, dir } => {
                let mut inner = self.lock();
                inner.state.game_mut(id).game_dir = dir;
                into_response(inner.state.save())
            }
            Request::TestTrigger { side, mode, params } => {
                let effect = ForceTrigger::common(TriggerSide::from_u8(side), mode, &params);
                self.note_effects(std::slice::from_ref(&effect));
                self.device.send(DeviceCommand::Effect(effect));
                Response::Ok
            }
            Request::ResetTriggers => {
                self.device.send(DeviceCommand::Reset);
                Response::Ok
            }
            Request::SetSettings { settings } => {
                let mut inner = self.lock();
                // DS 模式开关走 SetDsMode，这里保留当前值
                let ds_mode = inner.state.settings.ds_mode;
                inner.state.settings = store::Settings { ds_mode, ..settings };
                let saved = inner.state.save();
                drop(inner);
                if ds_mode {
                    self.set_ds_mode(true);
                }
                into_response(saved)
            }
            Request::SetDsMode { enabled } => {
                let saved = {
                    let mut inner = self.lock();
                    inner.state.settings.ds_mode = enabled;
                    inner.state.save()
                };
                self.set_ds_mode(enabled);
                into_response(saved)
            }
        }
    }

    /// 按设置启动或关闭全局 DS 模式。
    pub fn set_ds_mode(&self, enabled: bool) {
        let settings = self.lock().state.settings.clone();
        let options = DsOptions {
            trigger_strength: settings.ds_trigger_strength.min(100),
            rumble_gain: settings.ds_rumble_gain.min(200),
        };
        let mut ds = self.ds_mode.lock().unwrap();
        if enabled {
            // 重新开启以应用新的力度设置
            ds.take();
            log::info!("开启全局 DS 模式（扳机力度 {}%，震动增益 {}%）", options.trigger_strength, options.rumble_gain);
            *ds = Some(DsMode::start(self.device.clone(), options));
        } else if ds.take().is_some() {
            log::info!("关闭全局 DS 模式");
        }
    }

    pub fn ds_mode_setting(&self) -> bool {
        self.lock().state.settings.ds_mode
    }

    fn status(&self) -> Status {
        let inner = self.lock();
        let settings = inner.state.settings.clone();
        Status {
            version: env!("CARGO_PKG_VERSION").into(),
            controllers: self.device.controllers.lock().unwrap().clone(),
            active: inner
                .sessions
                .iter()
                .map(|(id, s)| {
                    let game = inner.games.iter().find(|g| g.id == *id);
                    ActiveGame {
                        id: *id,
                        name: game.map(|g| g.display_name().to_owned()).unwrap_or_default(),
                        route: game.map(Game::route),
                        pid: s.pid,
                        mod_command: s.mod_command.clone(),
                        mod_pid: s.mod_proc.as_ref().map(Child::id),
                    }
                })
                .collect(),
            jobs: inner.jobs.iter().map(|(k, v)| (*k, v.clone())).collect(),
            udp_port: settings.udp_port,
            udp_listening: self.udp_listening.load(Ordering::Relaxed),
            ptrace_ok: self.ptrace.0,
            ptrace_note: self.ptrace.1.clone(),
            dotnet_ok: which(&settings.dotnet),
            wine_ok: which(&settings.wine),
            settings,
            ds_mode: self.ds_mode.lock().unwrap().as_ref().map(DsMode::snapshot).unwrap_or_default(),
            last_effects: inner.last_effects.iter().cloned().collect(),
            log: crate::logger::recent(),
        }
    }

    fn game_views(&self) -> Vec<GameView> {
        let inner = self.lock();
        inner
            .games
            .iter()
            .map(|g| {
                let state = inner.state.game(g.id);
                GameView {
                    route: g.route(),
                    update_available: state.update_available(g),
                    game_dir: (g.route() == Route::InGameMod)
                        .then(|| store::resolve_game_dir(g, &state))
                        .flatten(),
                    running: inner.sessions.contains_key(&g.id),
                    error: inner.errors.get(&g.id).cloned(),
                    game: g.clone(),
                    state,
                }
            })
            .collect()
    }
}

fn into_response(result: anyhow::Result<()>) -> Response {
    match result {
        Ok(()) => Response::Ok,
        Err(e) => Response::Error { message: format!("{e:#}") },
    }
}

/// 配置文件名按进程名大小写敏感匹配（XGameMonitor 用 `Equals`），所以要传列表里的原始写法。
fn original_case(game: &Game, lower: &str) -> String {
    std::iter::once(&game.process_game_name)
        .chain(&game.process_game_names)
        .map(|n| n.trim().trim_end_matches(".exe"))
        .find(|n| n.eq_ignore_ascii_case(lower))
        .unwrap_or(lower)
        .to_owned()
}

/// 官方 OnStartGameVibMode / OnCloseGameVibMode。
fn vibration_binds(game: &Game, on: bool) -> [GripBind; 2] {
    let parse = |s: &str| {
        let mut out = [0u8; 5];
        for (slot, v) in out.iter_mut().zip(s.split(',')) {
            *slot = v.trim().parse().unwrap_or(0);
        }
        out
    };
    let left = parse(&game.vib_params);
    let mut right = parse(&game.vib_params_right);
    if right.iter().all(|b| *b == 0) {
        right = left;
    }
    let (filter_r, scale_r) = if on {
        (
            if game.vib_filter_right > 0 { game.vib_filter_right } else { game.vib_filter },
            if game.pwm_scal_right > 0 { game.pwm_scal_right } else { game.pwm_scal },
        )
    } else {
        (game.vib_filter_right, game.pwm_scal_right)
    };
    let (bind_type, left, right) = if on { (game.vib_type as u8, left, right) } else { (0, [0; 5], [0; 5]) };
    let make = |side, filter: i64, scale: i64, p: [u8; 5]| GripBind {
        side,
        bind_type,
        filter: filter as u8,
        scale: scale as u8,
        params: [p[0], p[1], p[2], p[3]],
    };
    [
        make(TriggerSide::Left, game.vib_filter, game.pwm_scal, left),
        make(TriggerSide::Right, filter_r, scale_r, right),
    ]
}

/// mod 程序位置：`mods/<id>/<modPath>/<modName>`，找不到时在解压目录里按文件名搜。
fn find_mod_exe(game: &Game) -> Option<PathBuf> {
    let root = paths::mod_dir(game.id);
    let direct = root.join(game.mod_rel_dir()).join(&game.mod_name);
    if direct.is_file() {
        return Some(direct);
    }
    fn walk(dir: &Path, name: &str) -> Option<PathBuf> {
        for entry in std::fs::read_dir(dir).ok()?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = walk(&path, name) {
                    return Some(found);
                }
            } else if entry.file_name().to_string_lossy().eq_ignore_ascii_case(name) {
                return Some(path);
            }
        }
        None
    }
    walk(&root, &game.mod_name)
}

/// Linux 版 XHelper（libxhelper.so）的位置。
fn xhelper_shim() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("FLYDIGI_XHELPER") {
        return Some(PathBuf::from(p));
    }
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    [dir.join("../lib/flydigi-linux/libxhelper.so"), dir.join("libxhelper.so")]
        .into_iter()
        .find(|p| p.is_file())
}

/// 按官方 GameHelper.StartGameMod 的参数启动 mod：`7878 name=<进程名> port=7878`。
/// 框架依赖的 .NET 程序（有 `<名字>.runtimeconfig.json`）用 dotnet 跑，其余用 Wine。
fn spawn_mod(exe: &Path, process_name: &str, settings: &store::Settings) -> anyhow::Result<(Child, String)> {
    let dir = exe.parent().unwrap_or(Path::new("."));
    let stem = exe.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    let port = settings.udp_port.to_string();
    let args = [port.clone(), format!("name={process_name}"), format!("port={port}")];
    let dll = format!("{stem}.dll");
    let mut cmd = if dir.join(format!("{stem}.runtimeconfig.json")).is_file() && dir.join(&dll).is_file() {
        if dir.join("XHelper.dll").is_file() {
            let shim = xhelper_shim().ok_or_else(|| anyhow::anyhow!("找不到 Linux 版 XHelper（libxhelper.so）"))?;
            let link = dir.join("XHelper.dll.so");
            let _ = std::fs::remove_file(&link);
            std::os::unix::fs::symlink(&shim, &link)?;
        }
        let mut c = Command::new(&settings.dotnet);
        c.arg(&dll).args(&args).env("DOTNET_ROLL_FORWARD", "Major");
        c
    } else {
        let mut c = Command::new(&settings.wine);
        c.arg(exe.file_name().unwrap_or_default())
            .args(&args)
            .env("WINEPREFIX", paths::wine_prefix())
            .env("WINEDEBUG", "-all");
        c
    };
    cmd.current_dir(dir).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).process_group(0);
    let command = format!("{:?} {}", cmd.get_program(), cmd.get_args().map(|a| a.to_string_lossy()).collect::<Vec<_>>().join(" "));
    let mut child = cmd.spawn()?;
    for stream in [child.stdout.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>), child.stderr.take().map(|s| Box::new(s) as _)]
        .into_iter()
        .flatten()
    {
        let tag = stem.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                log::info!("[{tag}] {line}");
            }
        });
    }
    Ok((child, command))
}

fn kill_group(child: &mut Child) {
    let pgid = child.id() as i32;
    // SAFETY: 给自己创建的进程组发信号。
    unsafe { libc::kill(-pgid, libc::SIGTERM) };
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    unsafe { libc::kill(-pgid, libc::SIGKILL) };
    let _ = child.wait();
}

fn which(program: &str) -> bool {
    if program.contains('/') {
        return Path::new(program).is_file();
    }
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|d| d.join(program).is_file()))
}
