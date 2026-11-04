//! 简单的中英双语：界面文字以中文为键，英文查表；没有收录的原样显示中文。
//! 默认跟随系统语言（LC_ALL / LC_MESSAGES / LANG），也可以在「服务」页单独切换，保存在 GUI 自己的配置里。

use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LangPref {
    #[default]
    Auto,
    Zh,
    En,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct GuiConfig {
    language: LangPref,
}

static ENGLISH: AtomicBool = AtomicBool::new(false);

fn config_path() -> std::path::PathBuf {
    flydigi_core::paths::config_dir().join("gui.json")
}

fn system_is_chinese() -> bool {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .find(|v| !v.is_empty())
        .is_some_and(|v| v.to_lowercase().starts_with("zh"))
}

fn apply(pref: LangPref) {
    let english = match pref {
        LangPref::Auto => !system_is_chinese(),
        LangPref::Zh => false,
        LangPref::En => true,
    };
    ENGLISH.store(english, Ordering::Relaxed);
}

/// 读取保存的语言设置并生效，返回当前设置。
pub fn init() -> LangPref {
    let pref = std::fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str::<GuiConfig>(&s).ok())
        .unwrap_or_default()
        .language;
    apply(pref);
    pref
}

pub fn set(pref: LangPref) {
    apply(pref);
    let config = GuiConfig { language: pref };
    if let Ok(json) = serde_json::to_string_pretty(&config) {
        let _ = flydigi_core::paths::write_atomic(&config_path(), json.as_bytes());
    }
}

pub fn is_english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

pub fn tr(zh: &'static str) -> &'static str {
    if !ENGLISH.load(Ordering::Relaxed) {
        return zh;
    }
    english(zh).unwrap_or(zh)
}

fn english(zh: &str) -> Option<&'static str> {
    Some(match zh {
        // 框架与导航
        "飞智空间站" => "Flydigi Space Station",
        "飞智空间站 Linux" => "Flydigi Space Station for Linux",
        "扳机" => "TRIGGERS",
        "手柄" => "CONTROLLER",
        "系统" => "SYSTEM",
        "扳机适配" => "Adaptive Triggers",
        "DS 模式" => "DS Mode",
        "扳机测试" => "Trigger Test",
        "按键映射" => "Button Mapping",
        "摇杆与体感" => "Sticks & Motion",
        "震动" => "Vibration",
        "灯效" => "Lighting",
        "宏" => "Macros",
        "屏幕" => "Screen",
        "服务" => "Service",
        "日志" => "Log",
        "尚未复刻" => "Not implemented yet",
        "官方空间站在这里提供：" => "Space Station offers here:",
        "当前版本优先实现力回馈扳机，其他功能会陆续加入。" => "Force-feedback triggers come first; other features will follow.",
        "按键映射、背键与组合键、板载配置 1–4 切换" => "button mapping, back paddles and combos, onboard profiles 1–4",
        "摇杆死区与曲线、体感（陀螺仪）映射" => "stick dead zones and curves, gyro mapping",
        "握把震动强度与扳机震动设置" => "grip vibration strength and trigger vibration",
        "手柄与充电座灯效" => "controller and charging dock lighting",
        "宏录制与绑定" => "macro recording and binding",
        "APEX 5 屏幕图片与动图上传" => "APEX 5 screen image and GIF upload",
        "飞智手柄" => "Flydigi controller",
        "未连接手柄" => "No controller connected",
        "DS 模式运行中" => "DS mode running",
        "DS 模式异常" => "DS mode error",
        "无法连接 flydigid 服务：" => "Cannot reach the flydigid service:",
        "操作失败：" => "Failed:",
        // 路线与效果
        "震动绑定" => "Vibration bind",
        "内存监控" => "Memory monitor",
        "遥测" => "Telemetry",
        "外置程序" => "External mod",
        "游戏内 mod" => "In-game mod",
        "仅 DS 模式" => "DS mode only",
        "普通" => "Normal",
        "赛车" => "Racing",
        "机枪" => "Machine gun",
        "狙击" => "Sniper",
        "锁定" => "Lock",
        "左" => "L",
        "右" => "R",
        // 扳机适配
        "🎯  扳机适配" => "🎯  Adaptive Triggers",
        "飞智官方适配的游戏：" => "Games supported by Flydigi:",
        "⟳ 更新游戏列表" => "⟳ Refresh list",
        "🔍 搜索游戏" => "🔍 Search games",
        "全部" => "All",
        "已启用" => "Enabled",
        "已安装" => "Installed",
        "可用" => "Available",
        "有更新" => "Update",
        "未安装" => "Not installed",
        "DS 已开启" => "DS on",
        "▶ 运行中" => "▶ Running",
        "〰 附带震动绑定" => "〰 Plus vibration bind",
        "游戏运行时让扳机跟随握把震动（命令 82），不需要 mod。" => "While the game runs, the triggers follow grip rumble (command 82). No mod needed.",
        "官方 XGameMonitor 按 JSON 配置读取游戏内存，根据角色动作切换扳机效果。用 dotnet 运行原版程序，XHelper.dll 换成 Linux 版。" => "Flydigi's XGameMonitor reads game memory per its JSON config and switches trigger effects by character action. The original program runs on dotnet with a Linux XHelper.dll.",
        "官方 ForzaDualSense 接收游戏的 Data Out 遥测并转换成扳机效果，用 dotnet 运行原版程序。" => "Flydigi's ForzaDualSense turns the game's Data Out telemetry into trigger effects. The original program runs on dotnet.",
        "官方外置 mod 程序：原生 Windows 程序用 Wine 运行，.NET 程序用 dotnet，经 UDP 7878 发送扳机指令。" => "Flydigi's external mod: native Windows programs run in Wine, .NET ones on dotnet; it sends trigger commands to UDP 7878.",
        "官方 mod 文件放进游戏目录，在游戏进程内经 UDP 7878 发送扳机指令。被覆盖的原文件会备份，卸载时还原。" => "Flydigi's mod files go into the game folder and send trigger commands to UDP 7878 from inside the game. Overwritten files are backed up and restored on uninstall.",
        "官方只提供 DS 模式。开启全局 DS 模式后，游戏会把手柄当成 DualSense，并直接发送自适应扳机效果。" => "Flydigi only offers DS mode here. With global DS mode on, the game sees a DualSense and sends adaptive trigger effects directly.",
        "游戏内设置：HUD 与游戏玩法 → 数据输出：开，IP 127.0.0.1，端口 5300。" => "In game: HUD and Gameplay → Data Out: On, IP 127.0.0.1, port 5300.",
        "带反作弊的游戏请按官方说明离线运行（关闭 EAC 等）。" => "Games with anti-cheat should run offline as Flydigi instructs (EAC off, etc.).",
        "全局 DS 模式未开启，可以在左侧「DS 模式」页打开。" => "Global DS mode is off. Turn it on in the DS Mode page.",
        "找不到 dotnet，请在「服务」页设置" => "dotnet not found; set it in the Service page",
        "找不到 wine；原生 Windows 程序的 mod 将无法运行" => "wine not found; mods that are native Windows programs will not run",
        "信息" => "Info",
        "注意事项" => "Notes",
        "更新说明" => "Changelog",
        "进程名" => "Process",
        "官方 mod" => "Flydigi mod",
        "最新版本" => "Latest",
        "已装版本" => "Installed",
        "游戏目录" => "Game folder",
        "mod 会解压到这里，默认按 Steam 库自动查找" => "The mod is extracted here; found from Steam libraries by default",
        "未找到，请手动填写" => "Not found, enter it manually",
        "保存目录" => "Save folder",
        "改回自动查找" => "Detect automatically",
        "操作" => "Actions",
        "已启用：检测到游戏时自动生效" => "Enabled: applied automatically when the game runs",
        "未启用" => "Disabled",
        "⬇ 下载安装" => "⬇ Install",
        "⬆ 更新" => "⬆ Update",
        "⟳ 重新安装" => "⟳ Reinstall",
        "🗑 卸载" => "🗑 Uninstall",
        "处理中…" => "Working…",
        "运行状态" => "Status",
        "游戏 pid" => "Game pid",
        // DS 模式
        "🎮  DS 模式" => "🎮  DS Mode",
        "虚拟一个 DualSense 手柄给所有游戏，作为没有官方 mod 的游戏的备用方案" => "Presents a virtual DualSense to every game, as a fallback for games without a Flydigi mod",
        "flydigid 服务未运行" => "The flydigid service is not running",
        "全局 DS 模式" => "Global DS mode",
        "运行中：游戏会看到一个 DualSense" => "Running: games see a DualSense",
        "已开启，但虚拟手柄没有创建成功" => "On, but the virtual controller could not be created",
        "已关闭" => "Off",
        "虚拟 DualSense" => "Virtual DualSense",
        "实体手柄已独占" => "Physical pad grabbed",
        "体感数据" => "Motion data",
        "已被程序打开" => "Opened by an app",
        "收到游戏输出报告" => "Output reports from games:",
        "力度" => "Strength",
        "扳机力度" => "Trigger strength",
        "握把震动增益" => "Grip rumble gain",
        "应用" => "Apply",
        "游戏设置" => "Game setup",
        "1. Steam 里对这个游戏关闭 Steam 输入，否则 DualSense 的自适应扳机指令到不了虚拟手柄。" => "1. Disable Steam Input for the game, or DualSense trigger commands never reach the virtual controller.",
        "2. 在启动选项里填下面这行，让游戏忽略实体手柄、只用虚拟 DualSense：" => "2. Add this to the launch options so the game ignores the physical pad and uses only the virtual DualSense:",
        "📋 复制" => "📋 Copy",
        "参照 ApexSenseBridge：按键和摇杆沿用手柄当前的板载配置，体感来自手柄的原始输入流；自适应扳机按 ASB 的规则翻译成飞智的扳机效果。" => "Modelled on ApexSenseBridge: buttons and sticks keep the pad's onboard profile, motion comes from the raw input stream, and adaptive triggers are translated with ASB's rules.",
        "暂不支持 DualSense 的音频触觉、触摸板滑动和灯条；实体手柄的视图键对应触摸板按下。" => "DualSense audio haptics, touchpad swipes and the lightbar are not supported yet. The View button acts as the touchpad click.",
        // 扳机测试
        "🔧  扳机测试" => "🔧  Trigger Test",
        "直接下发扳机效果，只改手柄的实时状态，不写入手柄存储" => "Send trigger effects directly. Only the live state changes; nothing is saved to the controller",
        "已连接的手柄" => "Connected controllers",
        "没有检测到飞智手柄的配置接口（usage page 0xFFA0），检查连接和 hidraw 权限。" => "No Flydigi configuration interface (usage page 0xFFA0) found. Check the connection and hidraw permissions.",
        "设备代码" => "device code",
        "扳机效果" => "Trigger effect",
        "◀ 左扳机 L2" => "◀ Left trigger L2",
        "右扳机 R2 ▶" => "Right trigger R2 ▶",
        "没有阻力，恢复普通扳机。" => "No resistance; a normal trigger.",
        "从起始行程开始持续阻力，赛车油门/刹车的手感。" => "Constant resistance from the start point, like a racing throttle or brake.",
        "越过起始行程后持续震颤，像机枪连射。" => "Rattles past the start point, like a machine gun.",
        "先阻挡、到断点后突然松开，像扣下狙击枪扳机。" => "Resists, then gives way at the break point, like a sniper trigger.",
        "在锁定位置卡死，扳机轴变成只有按下/松开两档。" => "Stops dead at the lock point; the axis becomes on/off only.",
        "官方软件从未使用过的模式，效果不确定。" => "A mode Flydigi's software never uses; behaviour is uncertain.",
        "起始行程" => "Start",
        "阻力" => "Resistance",
        "跟随行程" => "Match input",
        "压力档" => "Pressure",
        "强度" => "Strength",
        "频率" => "Frequency",
        "断点行程" => "Break length",
        "锁定位置" => "Lock point",
        "应用到手柄" => "Send to controller",
        "两侧恢复普通" => "Reset both",
        "游戏运行时，mod 或 DS 模式发来的指令会覆盖这里的设置。" => "While a game runs, commands from mods or DS mode override these settings.",
        "最近的扳机指令" => "Recent trigger commands",
        "还没有收到指令" => "No commands yet",
        // 服务
        "⚙  服务" => "⚙  Service",
        "flydigid 在后台复刻 Space Station 服务：接收 mod 指令、检测游戏、启停官方 mod" => "flydigid replaces the Space Station service: it receives mod commands, detects games and starts/stops Flydigi's mods",
        "flydigid 未运行。启动方式：systemctl --user start flydigid，或直接运行 flydigid。" => "flydigid is not running. Start it with systemctl --user start flydigid, or run flydigid directly.",
        "mod 指令端口 UDP" => "Mod command port UDP",
        "所有官方 mod 都把扳机指令发到这里" => "All Flydigi mods send trigger commands here",
        "运行 ForzaDualSense、XGameMonitor" => "Runs ForzaDualSense and XGameMonitor",
        "运行 F1 等原生 Windows mod" => "Runs native Windows mods such as F1",
        "读取游戏内存" => "Read game memory",
        "XGameMonitor 需要" => "Needed by XGameMonitor",
        "内存监控类游戏需要 sysctl kernel.yama.ptrace_scope=0，或给 flydigid 加 cap_sys_ptrace+eip 文件能力（NixOS 模块会自动配置）。" => "Memory-monitor games need sysctl kernel.yama.ptrace_scope=0, or the cap_sys_ptrace+eip file capability on flydigid (the NixOS module sets this up).",
        "版本" => "Version",
        "运行环境" => "Runtime",
        "dotnet 命令" => "dotnet command",
        "wine 命令" => "wine command",
        "保存" => "Save",
        "还原" => "Revert",
        "界面" => "Interface",
        "界面语言" => "Language",
        "跟随系统" => "System",
        "📄  日志" => "📄  Log",
        "flydigid 最近的日志" => "Recent flydigid log",
        _ => return None,
    })
}
