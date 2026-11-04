//! hidraw 设备发现与读写。

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use crate::effect::{ForceTrigger, GripBind, TriggerSide, rumble_report};
use crate::{REPORT_IN, REPORT_SIZE, VENDOR_ID, build_checksummed, build_report, cmd, is_reply};

/// 不走这套协议的产品（ControllerHidManager.FindSpecialHidDevice 里的排除项）。
const EXCLUDED_PIDS: [u16; 2] = [0x2503, 0x2F05];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    pub node: PathBuf,
    pub vendor_id: u16,
    pub product_id: u16,
    pub name: String,
    pub uniq: String,
}

/// 找出所有飞智手柄的 0xFFA0 vendor 接口。
pub fn discover() -> Vec<DeviceInfo> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir("/sys/class/hidraw") else {
        return found;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let dir = entry.path().join("device");
        let (Ok(uevent), Ok(desc)) = (
            std::fs::read_to_string(dir.join("uevent")),
            std::fs::read(dir.join("report_descriptor")),
        ) else {
            continue;
        };
        let field = |key: &str| {
            uevent
                .lines()
                .find_map(|l| l.strip_prefix(key))
                .unwrap_or_default()
                .to_string()
        };
        let hid_id = field("HID_ID=");
        let ids: Vec<u32> = hid_id.split(':').map(|s| u32::from_str_radix(s, 16).unwrap_or(0)).collect();
        let (vid, pid) = (ids.get(1).copied().unwrap_or(0) as u16, ids.get(2).copied().unwrap_or(0) as u16);
        if vid != VENDOR_ID || pid >> 12 != 2 || EXCLUDED_PIDS.contains(&pid) {
            continue;
        }
        if !desc.starts_with(&[0x06, 0xA0, 0xFF]) {
            continue;
        }
        found.push(DeviceInfo {
            node: Path::new("/dev").join(entry.file_name()),
            vendor_id: vid,
            product_id: pid,
            name: field("HID_NAME="),
            uniq: field("HID_UNIQ="),
        });
    }
    found
}

pub struct Controller {
    pub info: DeviceInfo,
    file: File,
}

impl Controller {
    pub fn open(info: DeviceInfo) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&info.node)
            .with_context(|| format!("打开 {}", info.node.display()))?;
        Ok(Self { info, file })
    }

    pub fn write_report(&mut self, report: &[u8; REPORT_SIZE]) -> Result<()> {
        self.file.write_all(report).context("写入 hidraw")
    }

    /// 丢掉已排队的输入报告。应答会广播给所有读者，不清空就可能读到别人的。
    fn drain(&mut self) {
        let mut buf = [0u8; 64];
        while matches!(self.file.read(&mut buf), Ok(n) if n > 0) {}
    }

    fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> Option<usize> {
        let mut pfd = libc::pollfd { fd: self.file.as_raw_fd(), events: libc::POLLIN, revents: 0 };
        let ms = timeout.as_millis().min(i32::MAX as u128) as i32;
        // SAFETY: pfd 是有效的单元素数组，fd 在 self.file 存活期间有效。
        let n = unsafe { libc::poll(&mut pfd, 1, ms) };
        if n <= 0 {
            return None;
        }
        self.file.read(buf).ok().filter(|n| *n > 0)
    }

    /// 发送一条命令并等它的应答。
    pub fn request(&mut self, report: &[u8; REPORT_SIZE], timeout: Duration) -> Result<[u8; 64]> {
        self.drain();
        self.write_report(report)?;
        let deadline = Instant::now() + timeout;
        let mut buf = [0u8; 64];
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            if let Some(n) = self.read_timeout(&mut buf, left) {
                if n >= 4 && buf[0] == REPORT_IN && is_reply(&buf[..n], report[3]) {
                    return Ok(buf);
                }
            }
        }
        bail!("命令 0x{:02X} 没有应答", report[3])
    }

    /// 命令 1：设备信息，`[6]` 是设备代码（Apex 5 为 128）。
    pub fn device_code(&mut self) -> Result<u8> {
        let reply = self.request(&build_report(cmd::GET_INFO, &[]), Duration::from_millis(500))?;
        Ok(reply[6])
    }

    /// 命令 0x10：当前输入传输方式 (controllerData, rawData)。
    pub fn input_transport(&mut self) -> Result<(bool, bool)> {
        let reply = self.request(&build_checksummed(cmd::READ_INPUT_TRANSPORT, &[]), Duration::from_millis(500))?;
        Ok((reply[6] == 1, reply[7] == 1))
    }

    /// 命令 0x11：设置输入传输方式。键盘、鼠标、第三方控制三项用 0xFF 保持不变。
    pub fn set_input_transport(&mut self, controller_data: bool, raw_data: bool) -> Result<()> {
        let payload = [controller_data as u8, raw_data as u8, 0xFF, 0xFF, 0xFF];
        self.request(&build_checksummed(cmd::SET_INPUT_TRANSPORT, &payload), Duration::from_millis(500))?;
        Ok(())
    }

    /// 非阻塞读一个输入报告。
    pub fn read_report(&mut self, buf: &mut [u8]) -> Option<usize> {
        self.file.read(buf).ok().filter(|n| *n > 0)
    }

    pub fn raw_fd(&self) -> std::os::fd::RawFd {
        self.file.as_raw_fd()
    }

    pub fn set_trigger(&mut self, effect: &ForceTrigger) -> Result<()> {
        self.write_report(&effect.report(false))
    }

    pub fn bind_grip(&mut self, bind: &GripBind) -> Result<()> {
        self.write_report(&bind.report())
    }

    pub fn rumble(&mut self, left: u8, right: u8) -> Result<()> {
        self.write_report(&rumble_report(left, right))
    }

    /// 两侧扳机恢复普通模式。Both=3 手柄会忽略，必须分两条发。
    pub fn reset_triggers(&mut self) -> Result<()> {
        self.set_trigger(&ForceTrigger::normal(TriggerSide::Left))?;
        std::thread::sleep(Duration::from_millis(10));
        self.set_trigger(&ForceTrigger::normal(TriggerSide::Right))
    }
}
