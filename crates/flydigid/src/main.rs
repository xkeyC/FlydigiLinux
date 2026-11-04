//! flydigid：Linux 版飞智空间站服务。
//!
//! 复刻 SpaceStationService 的扳机适配部分：监听官方 mod 的 UDP 指令并写给手柄，检测适配游戏
//! 并启停官方 mod / 震动绑定，通过 Unix socket 给 GUI 提供状态和管理接口。

mod caps;
mod daemon;
mod device;
mod dsmode;
mod logger;
mod udp;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use flydigi_core::ipc::{Request, Response};
use flydigi_core::paths;

use crate::daemon::Daemon;

static SIGNALLED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: libc::c_int) {
    SIGNALLED.store(true, Ordering::Relaxed);
}

fn serve_client(daemon: &Arc<Daemon>, stream: UnixStream) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;
    let mut line = String::new();
    while reader.read_line(&mut line)? > 0 {
        let response = match serde_json::from_str::<Request>(&line) {
            Ok(request) => daemon.handle(request),
            Err(e) => Response::Error { message: format!("无法解析请求：{e}") },
        };
        let mut out = serde_json::to_string(&response)?;
        out.push('\n');
        writer.write_all(out.as_bytes())?;
        line.clear();
    }
    Ok(())
}

fn main() -> Result<()> {
    logger::init();
    log::info!("flydigid {} 启动", env!("CARGO_PKG_VERSION"));

    let ptrace = caps::setup_ptrace();
    log::info!("{}", ptrace.1);

    // SAFETY: 处理函数只写一个原子变量。
    unsafe {
        libc::signal(libc::SIGTERM, on_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGINT, on_signal as *const () as libc::sighandler_t);
    }

    let device = device::spawn();
    let daemon = Daemon::new(device, ptrace);
    udp::spawn(daemon.clone(), daemon.udp_port());
    if daemon.ds_mode_setting() {
        daemon.set_ds_mode(true);
    }
    if !daemon.has_game_list() {
        daemon.refresh_list();
    }
    {
        let daemon = daemon.clone();
        std::thread::Builder::new().name("watch".into()).spawn(move || daemon.watch_loop())?;
    }

    let socket = paths::socket_path();
    if let Some(dir) = socket.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if UnixStream::connect(&socket).is_ok() {
        anyhow::bail!("{} 已有服务在运行", socket.display());
    }
    let _ = std::fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket).with_context(|| format!("监听 {}", socket.display()))?;
    listener.set_nonblocking(true)?;
    log::info!("IPC：{}", socket.display());

    while !SIGNALLED.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false)?;
                let daemon = daemon.clone();
                std::thread::spawn(move || {
                    if let Err(e) = serve_client(&daemon, stream) {
                        log::debug!("IPC 连接出错：{e:#}");
                    }
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => log::warn!("IPC accept：{e}"),
        }
    }

    log::info!("收到退出信号，停止所有会话并恢复扳机");
    daemon.shutdown.store(true, Ordering::Relaxed);
    daemon.stop_all();
    // 等手柄线程把复位命令写完
    std::thread::sleep(Duration::from_millis(300));
    let _ = std::fs::remove_file(&socket);
    Ok(())
}
