//! FlydigiLinux 的平台无关部分：官方适配游戏列表、官方 mod 的下载安装与启动方式、
//! DSX 指令解析，以及服务和 GUI 之间的 IPC 协议。
//!
//! 原则是尽量运行飞智的原版 mod：.NET mod 用 Linux 的 dotnet 跑，原生 Windows mod 用 Wine 跑，
//! 本项目只复刻 Space Station 服务端（SpaceStationService）那一侧的行为。

pub mod ds5;
pub mod dsx;
pub mod gamelist;
pub mod ipc;
pub mod paths;
pub mod procscan;
pub mod steam;
pub mod store;

pub use gamelist::{Game, Route};
