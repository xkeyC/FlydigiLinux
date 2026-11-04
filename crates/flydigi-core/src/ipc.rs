//! 服务（flydigid）与 GUI 之间的协议：Unix socket 上一行一个 JSON 请求、一行一个 JSON 应答。
//! 状态只由服务持有和修改，GUI 只发请求、轮询状态。

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::gamelist::{Game, Route};
use crate::paths;
use crate::store::{GameState, Settings};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Status,
    Games,
    RefreshList,
    Install { id: i64 },
    Uninstall { id: i64 },
    SetEnabled { id: i64, enabled: bool },
    SetGameDir { id: i64, dir: Option<PathBuf> },
    TestTrigger { side: u8, mode: u8, params: Vec<u8> },
    ResetTriggers,
    SetSettings { settings: Settings },
    SetDsMode { enabled: bool },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Response {
    Ok,
    Error { message: String },
    Status(Status),
    Games { games: Vec<GameView> },
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ControllerView {
    pub node: String,
    pub name: String,
    pub product_id: u16,
    pub device_code: Option<u8>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ActiveGame {
    pub id: i64,
    pub name: String,
    pub route: Option<Route>,
    pub pid: u32,
    /// 正在运行的 mod 进程及其命令。
    pub mod_command: Option<String>,
    pub mod_pid: Option<u32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Status {
    pub version: String,
    pub controllers: Vec<ControllerView>,
    pub active: Vec<ActiveGame>,
    /// 后台任务（下载、刷新列表），游戏 id -> 描述；列表刷新用 id 0。
    pub jobs: Vec<(i64, String)>,
    pub udp_port: u16,
    pub udp_listening: bool,
    /// 能否读取其他进程内存（XGameMonitor 需要）。
    pub ptrace_ok: bool,
    pub ptrace_note: String,
    pub dotnet_ok: bool,
    pub wine_ok: bool,
    pub settings: Settings,
    pub ds_mode: DsModeStatus,
    /// 最近的扳机指令，「L|R 模式号 [参数]」。
    pub last_effects: Vec<String>,
    pub log: Vec<String>,
}

/// 全局 DS 模式（虚拟 DualSense）的运行状态。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DsModeStatus {
    pub enabled: bool,
    /// 虚拟 DualSense 已创建。
    pub active: bool,
    /// 已独占实体手柄的输入。
    pub pad_connected: bool,
    /// 已打开 vendor 原始输入流（体感）。
    pub motion: bool,
    /// 有程序（游戏、SDL、Steam）打开了虚拟手柄。
    pub game_opened: bool,
    /// 收到的输出报告数；一直为 0 说明游戏没有用这个虚拟手柄。
    pub output_reports: u64,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameView {
    pub game: Game,
    pub state: GameState,
    pub route: Route,
    pub update_available: bool,
    pub game_dir: Option<PathBuf>,
    pub running: bool,
    /// 最近一次安装/卸载失败的原因。
    pub error: Option<String>,
}

pub fn call(request: &Request) -> Result<Response> {
    let path = paths::socket_path();
    let mut stream = UnixStream::connect(&path).with_context(|| format!("连接服务 {}", path.display()))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut line = serde_json::to_string(request)?;
    line.push('\n');
    stream.write_all(line.as_bytes())?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply)?;
    let response: Response = serde_json::from_str(&reply).context("解析服务应答")?;
    if let Response::Error { message } = &response {
        bail!("{message}");
    }
    Ok(response)
}
