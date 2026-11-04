//! 飞智新协议（NewXInput / V2）手柄的 vendor HID 接口。
//!
//! 设备暴露 usage page 0xFFA0 的 hidraw 节点：输出报告 ID 3、输入报告 ID 4，均为 32 字节，
//! 帧格式 `03 5A A5 <cmd> <len> <payload…>`。协议细节参考 Space Station 的
//! Flydigi.ControllerSdk 与 OpenFlydigi 的 PROTOCOL.md。

pub mod device;
pub mod effect;

pub use device::{Controller, DeviceInfo, discover};
pub use effect::{ForceTrigger, GripBind, TriggerMode, TriggerSide};

pub const VENDOR_ID: u16 = 0x37D7;
pub const REPORT_OUT: u8 = 0x03;
pub const REPORT_IN: u8 = 0x04;
pub const MAGIC: [u8; 2] = [0x5A, 0xA5];
pub const REPORT_SIZE: usize = 32;

pub mod cmd {
    pub const GET_INFO: u8 = 0x01;
    pub const READ_INPUT_TRANSPORT: u8 = 0x10;
    pub const SET_INPUT_TRANSPORT: u8 = 0x11;
    /// 原始输入流（含体感），在打开 rawData 传输后持续上报。
    pub const OPERATOR_DATA: u8 = 0xEF;
    pub const RUMBLE: u8 = 0x12;
    pub const FORCE_TRIGGER: u8 = 0x51;
    pub const FORCE_TRIGGER_SYNC_GRIP: u8 = 0x52;
}

/// 组一个 32 字节输出报告。81/82/18 号命令不需要校验和，长度字节由调用方放在 payload 首位。
pub fn build_report(command: u8, payload: &[u8]) -> [u8; REPORT_SIZE] {
    let mut report = [0u8; REPORT_SIZE];
    report[0] = REPORT_OUT;
    report[1..3].copy_from_slice(&MAGIC);
    report[3] = command;
    let n = payload.len().min(REPORT_SIZE - 4);
    report[4..4 + n].copy_from_slice(&payload[..n]);
    report
}

/// 带校验和的命令：`[4]` 为 payload 长度 + 2，`[3+len]` 为 `[3, 3+len)` 的 8 位和。
pub fn build_checksummed(command: u8, payload: &[u8]) -> [u8; REPORT_SIZE] {
    let mut report = [0u8; REPORT_SIZE];
    report[0] = REPORT_OUT;
    report[1..3].copy_from_slice(&MAGIC);
    report[3] = command;
    let len = (payload.len() + 2).min(REPORT_SIZE - 4);
    report[4] = len as u8;
    report[5..5 + payload.len().min(REPORT_SIZE - 6)]
        .copy_from_slice(&payload[..payload.len().min(REPORT_SIZE - 6)]);
    let end = 3 + len;
    if end < REPORT_SIZE {
        report[end] = report[3..end].iter().fold(0u8, |a, b| a.wrapping_add(*b));
    }
    report
}

/// 输入报告是否是某条命令的应答（`04 5A A5 <cmd>`）。
pub fn is_reply(report: &[u8], command: u8) -> bool {
    report.len() >= 4 && report[0] == REPORT_IN && report[1..3] == MAGIC && report[3] == command
}
