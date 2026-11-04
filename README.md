# FlydigiLinux

Linux 版飞智空间站。当前目标是复刻 **力回馈扳机** 相关功能：获取飞智官方的适配游戏列表，下载、安装、启用、卸载官方 mod，并在游戏运行时驱动 APEX 5 的扳机；另提供全局 DS 模式作为没有官方 mod 时的备用方案。其他功能（按键映射、灯效、屏幕等）在界面里留了位置，后续再复刻。

原则是 **尽量运行飞智的原版 mod**，本项目只复刻 Space Station 服务端（SpaceStationService）那一侧。

## 组成

| crate | 作用 |
|---|---|
| `flydigid` | 后台服务：UDP 7878 接收官方 mod 的扳机指令并写给手柄；检测适配游戏并启停官方 mod 或震动绑定；全局 DS 模式；Unix socket 给 GUI 提供接口 |
| `flydigi-gui` | egui 图形界面，中英双语（随系统，可在「服务」页单独切换） |
| `flydigi-core` | 官方游戏列表、mod 安装卸载、DSX 指令解析、DualSense 输出报告翻译、IPC 协议 |
| `flydigi-proto` | 手柄 vendor HID 协议（usage page 0xFFA0，报告 3/4，`5A A5 <cmd>`） |
| `xhelper-shim` | 官方 XGameMonitor 依赖的 `XHelper.dll` 的 Linux 实现（`/proc/<pid>/mem`） |

## 各类适配游戏怎么跑

| 官方路线 | 例子 | Linux 上 |
|---|---|---|
| 震动绑定 | Apex 英雄、鸣潮、BF6 | flydigid 在游戏启动时发命令 82，让扳机跟随握把震动 |
| 外置 mod：ForzaDualSense | 地平线 4/5/6、极限竞速 | 原版 .NET 程序用 Linux 的 `dotnet` 运行（`DOTNET_ROLL_FORWARD=Major`），游戏内打开 Data Out 到 127.0.0.1:5300 |
| 外置 mod：XGameMonitor | 艾尔登法环、赛博朋克 2077、荒野大镖客 2、巫师 3 | 原版 .NET 程序用 `dotnet` 运行，旁边放 `XHelper.dll.so`（本项目的 shim），按官方 JSON 配置读游戏内存 |
| 外置 mod：原生 Windows 程序 | F1 23/24/25 | 用 wine 运行（独立前缀，只走 UDP） |
| 游戏内 mod | GTA5、生化危机 2/3/7、辐射 4 | 解压进游戏目录（按 Steam appid 自动找，原文件备份），游戏进程内经 UDP 7878 发指令 |
| 仅 DS 模式 | 地平线：西之绝境、战神等 | 开启全局 DS 模式 |

## 全局 DS 模式

参照 [ApexSenseBridge](https://github.com/ReynArts/ApexSenseBridge) 实现：

- 通过 `/dev/uhid` 虚拟一个 USB DualSense（描述符、特性报告取自真机）。
- 按键和摇杆来自 xpad 的 evdev（保留手柄板载配置的改键），并用 `EVIOCGRAB` 独占；体感与 PS 键来自 vendor HID 原始输入流（命令 0x11 打开 rawData，报告 0xEF），退出时恢复原设置。
- 游戏发来的自适应扳机效果按 ASB 的规则翻译成飞智扳机命令（包括弓效果按扳机位置切换），震动转成握把马达，力度和增益可调。
- 游戏需要关闭 Steam 输入，启动选项加 `SDL_GAMECONTROLLER_IGNORE_DEVICES=0x37d7/0x2501 SDL_JOYSTICK_IGNORE_DEVICES=0x37d7/0x2501 %command%`。
- 暂不支持音频触觉、触摸板滑动、灯条。

## 安装（NixOS）

```nix
{
  inputs.flydigi-linux.url = "github:xkeyC/FlydigiLinux";
  # …
  imports = [ inputs.flydigi-linux.nixosModules.default ];
  programs.flydigi-linux = {
    enable = true;
    wine = pkgs.wineWow64Packages.stable; # 可选，F1 等原生 Windows mod 需要
  };
}
```

模块会：安装包；加 udev 规则（vendor hidraw 和 `/dev/uhid` 给当前登录用户）；加载 `uhid`；用 `security.wrappers` 给 `flydigid` 加 `cap_sys_ptrace`（XGameMonitor 读内存需要，`ptrace_scope=1` 时必需）；注册 `systemctl --user` 服务 `flydigid`。

## 开发

```sh
nix develop
cargo build
FLYDIGI_XHELPER=$PWD/target/debug/libxhelper.so cargo run -p flydigid
cargo run -p flydigi-gui
```

数据位置：游戏列表与 mod 在 `~/.local/share/flydigi-linux/`，状态在 `~/.config/flydigi-linux/state.json`，GUI 语言在 `~/.config/flydigi-linux/gui.json`。

## 致谢与许可

- [ApexSenseBridge](https://github.com/ReynArts/ApexSenseBridge)（GPL-3.0-or-later）：DS 模式的整体设计、自适应扳机翻译、原始输入流解码。
- [OpenFlydigi](https://github.com/mkaliaha/openflydigi)（MIT）：协议文档、DualSense 描述符与特性报告、uhid 用法。
- 飞智的游戏列表、mod 和配置归飞智所有，本项目只在运行时从官方接口下载，不随仓库分发。

本项目以 GPL-3.0-or-later 发布。
