//! 全局 DS 模式：通过 /dev/uhid 虚拟一个 USB DualSense，作为没有官方 mod 的游戏的备用方案。
//!
//! 设计参照 ApexSenseBridge（GPL-3.0-or-later）：
//! - 输入：实体手柄映射后的按键/摇杆来自 xpad 的 evdev（对应 ASB 的 XInput，保留手柄板载配置的改键），
//!   体感和 PS 键来自 vendor HID 的原始输入流（命令 0x11 打开 rawData，报告 0xEF）。
//!   evdev 节点用 EVIOCGRAB 独占，相当于 ASB 用 HidHide 隐藏实体手柄。
//! - 输出：游戏发给 DualSense 的自适应扳机效果按 ASB 的规则翻译成飞智扳机命令，震动转成握把马达。
//!
//! 仍会枚举到实体手柄的游戏需要在启动选项里加
//! `SDL_GAMECONTROLLER_IGNORE_DEVICES=0x37d7/0x2501 SDL_JOYSTICK_IGNORE_DEVICES=0x37d7/0x2501`，
//! 并且关闭 Steam 输入。

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use flydigi_core::ds5::{self, Feedback, InputState, TriggerBridge, button};
use flydigi_core::ipc::DsModeStatus;
use flydigi_proto::{Controller, ForceTrigger, TriggerSide, cmd};

use crate::device::{DeviceCommand, DeviceHandle};
// ------------------------------------------------------------ uhid

const UHID_DESTROY: u32 = 1;
const UHID_OPEN: u32 = 4;
const UHID_CLOSE: u32 = 5;
const UHID_OUTPUT: u32 = 6;
const UHID_GET_REPORT: u32 = 9;
const UHID_GET_REPORT_REPLY: u32 = 10;
const UHID_CREATE2: u32 = 11;
const UHID_INPUT2: u32 = 12;
const UHID_SET_REPORT: u32 = 13;
const UHID_SET_REPORT_REPLY: u32 = 14;
const BUS_USB: u16 = 0x03;
/// `struct uhid_event` 的大小：4 字节类型 + 最大的 create2 联合体。
const EVENT_SIZE: usize = 4 + 128 + 64 + 64 + 2 + 2 + 4 * 4 + 4096;

struct Uhid {
    file: File,
}

enum UhidEvent {
    Output(Vec<u8>),
    Opened(bool),
}

impl Uhid {
    fn create() -> Result<Self> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open("/dev/uhid")
            .context("打开 /dev/uhid（需要 udev 规则给当前用户读写权限，并加载 uhid 模块）")?;
        let mut ev = vec![0u8; EVENT_SIZE];
        ev[0..4].copy_from_slice(&UHID_CREATE2.to_ne_bytes());
        ev[4..4 + ds5::NAME.len()].copy_from_slice(ds5::NAME.as_bytes());
        let phys = b"flydigi-linux/ds5";
        ev[132..132 + phys.len()].copy_from_slice(phys);
        ev[260..262].copy_from_slice(&(ds5::REPORT_DESCRIPTOR.len() as u16).to_ne_bytes());
        ev[262..264].copy_from_slice(&BUS_USB.to_ne_bytes());
        ev[264..268].copy_from_slice(&ds5::VENDOR_ID.to_ne_bytes());
        ev[268..272].copy_from_slice(&ds5::PRODUCT_ID.to_ne_bytes());
        ev[272..276].copy_from_slice(&0x8111u32.to_ne_bytes());
        ev[280..280 + ds5::REPORT_DESCRIPTOR.len()].copy_from_slice(ds5::REPORT_DESCRIPTOR);
        file.write_all(&ev).context("创建虚拟 DualSense")?;
        Ok(Self { file })
    }

    fn send_input(&mut self, report: &[u8]) -> Result<()> {
        let mut ev = vec![0u8; EVENT_SIZE];
        ev[0..4].copy_from_slice(&UHID_INPUT2.to_ne_bytes());
        ev[4..6].copy_from_slice(&(report.len() as u16).to_ne_bytes());
        ev[6..6 + report.len()].copy_from_slice(report);
        self.file.write_all(&ev).context("写入虚拟 DualSense 输入")
    }

    fn reply(&mut self, kind: u32, id: u32, err: u16, data: &[u8]) -> Result<()> {
        let mut ev = vec![0u8; EVENT_SIZE];
        ev[0..4].copy_from_slice(&kind.to_ne_bytes());
        ev[4..8].copy_from_slice(&id.to_ne_bytes());
        ev[8..10].copy_from_slice(&err.to_ne_bytes());
        if kind == UHID_GET_REPORT_REPLY {
            ev[10..12].copy_from_slice(&(data.len() as u16).to_ne_bytes());
            ev[12..12 + data.len()].copy_from_slice(data);
        }
        self.file.write_all(&ev)?;
        Ok(())
    }

    /// 读出所有排队的事件；GET/SET_REPORT 在这里直接应答。
    fn events(&mut self) -> Result<Vec<UhidEvent>> {
        let mut out = Vec::new();
        let mut ev = vec![0u8; EVENT_SIZE];
        loop {
            match self.file.read(&mut ev) {
                Ok(n) if n >= 4 => {}
                Ok(_) => break,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e.into()),
            }
            let u32_at = |o: usize| u32::from_ne_bytes(ev[o..o + 4].try_into().unwrap());
            let u16_at = |o: usize| u16::from_ne_bytes(ev[o..o + 2].try_into().unwrap()) as usize;
            match u32_at(0) {
                UHID_OUTPUT => {
                    let size = u16_at(4 + 4096).min(4096);
                    out.push(UhidEvent::Output(ev[4..4 + size].to_vec()));
                }
                UHID_OPEN => out.push(UhidEvent::Opened(true)),
                UHID_CLOSE => out.push(UhidEvent::Opened(false)),
                UHID_GET_REPORT => {
                    let (id, rnum) = (u32_at(4), ev[8]);
                    match ds5::feature_report(rnum) {
                        Some(body) => {
                            let mut data = vec![rnum];
                            data.extend_from_slice(body);
                            self.reply(UHID_GET_REPORT_REPLY, id, 0, &data)?;
                        }
                        None => self.reply(UHID_GET_REPORT_REPLY, id, libc::EIO as u16, &[])?,
                    }
                }
                UHID_SET_REPORT => {
                    let id = u32_at(4);
                    let size = u16_at(10).min(4096);
                    let data = ev[12..12 + size].to_vec();
                    self.reply(UHID_SET_REPORT_REPLY, id, 0, &[])?;
                    out.push(UhidEvent::Output(data));
                }
                _ => {}
            }
        }
        Ok(out)
    }
}

impl Drop for Uhid {
    fn drop(&mut self) {
        let mut ev = vec![0u8; EVENT_SIZE];
        ev[0..4].copy_from_slice(&UHID_DESTROY.to_ne_bytes());
        let _ = self.file.write_all(&ev);
    }
}


// ------------------------------------------------------------ evdev：映射后的按键与摇杆

const EV_SYN: u16 = 0;
const EV_KEY: u16 = 1;
const EV_ABS: u16 = 3;
const ABS_CODES: [u16; 8] = [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x10, 0x11];
const EVIOCGRAB: libc::c_ulong = 0x4004_4590;
/// XInput 的扳机按下阈值（ASB 的 kTriggerThreshold）。
const TRIGGER_THRESHOLD: u8 = 30;

fn eviocgabs(code: u16) -> libc::c_ulong {
    0x8018_4540 + code as libc::c_ulong
}

/// 实体手柄的 evdev 节点：飞智厂商号、带模拟轴的那个（排除键盘、鼠标子设备）。
fn find_pad_evdev() -> Option<PathBuf> {
    for entry in std::fs::read_dir("/sys/class/input").ok()?.flatten() {
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with("event") {
            continue;
        }
        let dev = entry.path().join("device");
        let vendor = std::fs::read_to_string(dev.join("id/vendor")).unwrap_or_default();
        let abs = std::fs::read_to_string(dev.join("capabilities/abs")).unwrap_or_default();
        if vendor.trim() == "37d7" && !matches!(abs.trim(), "" | "0") {
            return Some(PathBuf::from("/dev/input").join(name));
        }
    }
    None
}

struct Evdev {
    file: File,
    absinfo: HashMap<u16, (i32, i32)>,
    axes: HashMap<u16, i32>,
    keys: HashMap<u16, bool>,
}

impl Evdev {
    fn open() -> Result<Self> {
        let path = find_pad_evdev().context("没有找到实体手柄的 evdev 节点")?;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&path)
            .with_context(|| format!("打开 {}", path.display()))?;
        let fd = file.as_raw_fd();
        // SAFETY: EVIOCGRAB 和 EVIOCGABS 只读写传入的整数/absinfo 结构。
        if unsafe { libc::ioctl(fd, EVIOCGRAB as _, 1) } != 0 {
            bail!("独占 {} 失败（可能被其他程序独占）", path.display());
        }
        let mut absinfo = HashMap::new();
        for code in ABS_CODES {
            let mut info = [0i32; 6];
            if unsafe { libc::ioctl(fd, eviocgabs(code) as _, info.as_mut_ptr()) } == 0 {
                absinfo.insert(code, (info[1], info[2]));
            }
        }
        log::info!("DS 模式：已独占实体手柄输入 {}", path.display());
        Ok(Self { file, absinfo, axes: HashMap::new(), keys: HashMap::new() })
    }

    /// 读完排队的事件，返回是否遇到 SYN_REPORT；设备消失时返回错误。
    fn read(&mut self) -> Result<bool> {
        let mut buf = [0u8; 24 * 64];
        let mut synced = false;
        loop {
            let n = match self.file.read(&mut buf) {
                Ok(0) => bail!("手柄已断开"),
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(synced),
                Err(e) => return Err(e.into()),
            };
            for ev in buf[..n].chunks_exact(24) {
                let kind = u16::from_ne_bytes([ev[16], ev[17]]);
                let code = u16::from_ne_bytes([ev[18], ev[19]]);
                let value = i32::from_ne_bytes([ev[20], ev[21], ev[22], ev[23]]);
                match kind {
                    EV_KEY => {
                        self.keys.insert(code, value != 0);
                    }
                    EV_ABS => {
                        self.axes.insert(code, value);
                    }
                    EV_SYN if code == 0 => synced = true,
                    _ => {}
                }
            }
        }
    }

    fn axis(&self, code: u16, neutral: u8) -> u8 {
        match (self.absinfo.get(&code), self.axes.get(&code)) {
            (Some(&(min, max)), Some(&v)) if max > min => {
                (((v - min) as i64 * 255 + (max - min) as i64 / 2) / (max - min) as i64).clamp(0, 255) as u8
            }
            _ => neutral,
        }
    }

    fn pressed(&self, code: u16) -> bool {
        self.keys.get(&code).copied().unwrap_or(false)
    }

    /// 对应 ASB 的 mapXInputButtons：视图键当触摸板按下，菜单键当 Options。
    fn fill(&self, s: &mut InputState) {
        s.lx = self.axis(0x00, 0x80);
        s.ly = self.axis(0x01, 0x80);
        s.rx = self.axis(0x03, 0x80);
        s.ry = self.axis(0x04, 0x80);
        s.l2 = self.axis(0x02, 0);
        s.r2 = self.axis(0x05, 0);
        s.hat = ds5::hat(*self.axes.get(&0x10).unwrap_or(&0), *self.axes.get(&0x11).unwrap_or(&0));
        s.buttons = [0; 3];
        // xpad：BTN_SOUTH=A、BTN_EAST=B、BTN_X(0x133)=X、BTN_Y(0x134)=Y
        let map: [(u16, usize, u8); 11] = [
            (0x130, 0, button::CROSS),
            (0x131, 0, button::CIRCLE),
            (0x133, 0, button::SQUARE),
            (0x134, 0, button::TRIANGLE),
            (0x136, 1, button::L1),
            (0x137, 1, button::R1),
            (0x13b, 1, button::OPTIONS),
            (0x13d, 1, button::L3),
            (0x13e, 1, button::R3),
            (0x13c, 2, button::PS),
            (0x13a, 2, button::TOUCHPAD),
        ];
        for (code, group, mask) in map {
            s.set(group, mask, self.pressed(code));
        }
        s.set(1, button::L2, s.l2 > TRIGGER_THRESHOLD);
        s.set(1, button::R2, s.r2 > TRIGGER_THRESHOLD);
    }
}

// ------------------------------------------------------------ vendor 原始输入流：体感与 PS 键

/// 0xEF 报告里 ASB 用到的字段（decodeApex5InputReport）。
struct VendorInput {
    state: InputState,
    at: Instant,
}

fn i16_at(r: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([r[o], r[o + 1]])
}

/// XInput 风格的有符号摇杆值转成 DualSense 的 0..255，Y 轴向上为正所以要反转。
fn axis_to_byte(value: i16, invert: bool) -> u8 {
    let v = ((value as i32 + 32768) as u32 * 255 + 32767) / 65535;
    if invert { 255 - v as u8 } else { v as u8 }
}

fn decode_vendor(r: &[u8]) -> Option<InputState> {
    if r.len() < 30 || r[0] != flydigi_proto::REPORT_IN || r[1..3] != flydigi_proto::MAGIC || r[3] != cmd::OPERATOR_DATA {
        return None;
    }
    let mut s = InputState::default();
    s.lx = axis_to_byte(i16_at(r, 4), false);
    s.ly = axis_to_byte(i16_at(r, 6), true);
    s.rx = axis_to_byte(i16_at(r, 8), false);
    s.ry = axis_to_byte(i16_at(r, 10), true);
    s.l2 = r[16];
    s.r2 = r[17];
    let (p, q) = (r[12], r[13]);
    let bit = |b: u8, m: u8| b & m != 0;
    s.hat = ds5::hat(
        bit(p, 0x02) as i32 - bit(p, 0x08) as i32,
        bit(p, 0x04) as i32 - bit(p, 0x01) as i32,
    );
    s.set(0, button::CROSS, bit(p, 0x10));
    s.set(0, button::CIRCLE, bit(p, 0x20));
    s.set(2, button::TOUCHPAD, bit(p, 0x40));
    s.set(0, button::SQUARE, bit(p, 0x80));
    s.set(0, button::TRIANGLE, bit(q, 0x01));
    s.set(1, button::OPTIONS, bit(q, 0x02));
    s.set(1, button::L1, bit(q, 0x04));
    s.set(1, button::R1, bit(q, 0x08));
    s.set(1, button::L3, bit(q, 0x40));
    s.set(1, button::R3, bit(q, 0x80));
    s.set(1, button::L2, s.l2 > TRIGGER_THRESHOLD);
    s.set(1, button::R2, s.r2 > TRIGGER_THRESHOLD);
    s.set(2, button::PS, r[15] & 0x08 != 0);
    s.gyro = [i16_at(r, 18), i16_at(r, 20), i16_at(r, 22)];
    // APEX 5 约 4096 单位/g，虚拟 DualSense 的标定是 10000 单位/g
    let scale = |v: i16| (v as i32 * 625 / 256).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    s.accel = [scale(i16_at(r, 24)), scale(i16_at(r, 26)), scale(i16_at(r, 28))];
    Some(s)
}

/// 独立打开 vendor 接口读原始流；打开时记下原来的传输设置，关闭时恢复。
struct Vendor {
    ctl: Controller,
    original: Option<(bool, bool)>,
}

impl Vendor {
    fn open() -> Result<Self> {
        let info = flydigi_proto::discover().into_iter().next().context("没有找到手柄的 vendor 接口")?;
        let mut ctl = Controller::open(info)?;
        let original = ctl.input_transport().ok();
        ctl.set_input_transport(original.is_none_or(|o| o.0), true)?;
        log::info!("DS 模式：已打开原始输入流（体感），原设置 {original:?}");
        Ok(Self { ctl, original })
    }
}

impl Drop for Vendor {
    fn drop(&mut self) {
        if let Some((controller, raw)) = self.original {
            if let Err(e) = self.ctl.set_input_transport(controller, raw) {
                log::warn!("DS 模式：恢复手柄输入传输设置失败：{e:#}");
            }
        }
    }
}

// ------------------------------------------------------------ 运行

#[derive(Clone, Copy)]
pub struct DsOptions {
    /// 扳机力度 0..=100%
    pub trigger_strength: u32,
    /// 握把震动增益 0..=200%
    pub rumble_gain: u32,
}

pub struct DsMode {
    stop: Arc<AtomicBool>,
    pub status: Arc<Mutex<DsModeStatus>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl DsMode {
    pub fn start(device: DeviceHandle, options: DsOptions) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new(DsModeStatus { enabled: true, ..Default::default() }));
        let thread = {
            let (stop, status) = (stop.clone(), status.clone());
            std::thread::Builder::new()
                .name("dsmode".into())
                .spawn(move || {
                    if let Err(e) = run(&device, options, &stop, &status) {
                        log::error!("DS 模式：{e:#}");
                        status.lock().unwrap().error = Some(format!("{e:#}"));
                    }
                    status.lock().unwrap().active = false;
                })
                .expect("启动 DS 模式线程")
        };
        Self { stop, status, thread: Some(thread) }
    }

    pub fn snapshot(&self) -> DsModeStatus {
        self.status.lock().unwrap().clone()
    }
}

impl Drop for DsMode {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// 体感数据多久没更新就当失效（手柄休眠、流被关掉）。
const VENDOR_FRESHNESS: Duration = Duration::from_millis(100);

fn compose(evdev: Option<&Evdev>, vendor: Option<&VendorInput>, state: &mut InputState) {
    let fresh = vendor.filter(|v| v.at.elapsed() <= VENDOR_FRESHNESS);
    match (evdev, fresh) {
        (Some(e), v) => {
            e.fill(state);
            if let Some(v) = v {
                state.gyro = v.state.gyro;
                state.accel = v.state.accel;
                state.buttons[2] |= v.state.buttons[2] & button::PS;
            } else {
                state.gyro = [0; 3];
                state.accel = [0; 3];
            }
        }
        (None, Some(v)) => {
            state.lx = v.state.lx;
            state.ly = v.state.ly;
            state.rx = v.state.rx;
            state.ry = v.state.ry;
            state.l2 = v.state.l2;
            state.r2 = v.state.r2;
            state.hat = v.state.hat;
            state.buttons = v.state.buttons;
            state.gyro = v.state.gyro;
            state.accel = v.state.accel;
        }
        (None, None) => state.release(),
    }
}

fn run(device: &DeviceHandle, options: DsOptions, stop: &AtomicBool, status: &Mutex<DsModeStatus>) -> Result<()> {
    let mut uhid = Uhid::create()?;
    log::info!("DS 模式：虚拟 DualSense 已创建");
    status.lock().unwrap().active = true;

    let mut state = InputState::default();
    let mut bridge = TriggerBridge::default();
    let mut evdev: Option<Evdev> = None;
    let mut vendor: Option<Vendor> = None;
    let mut vendor_input: Option<VendorInput> = None;
    let mut next_open = Instant::now();
    let mut last_rumble = None;
    let send_trigger = |side: TriggerSide, (mode, params): ds5::TriggerCommand| {
        // 实时路径和官方一样走 ForceTriggerConfigCommon（含 Race 的改写）
        device.send(DeviceCommand::Effect(ForceTrigger::common(side, mode, &params)));
    };
    uhid.send_input(&state.pack())?;

    while !stop.load(Ordering::Relaxed) {
        if (evdev.is_none() || vendor.is_none()) && Instant::now() >= next_open {
            let reconnect = evdev.is_none() && vendor.is_none();
            if evdev.is_none() {
                evdev = Evdev::open().map_err(|e| log::debug!("DS 模式：{e:#}")).ok();
            }
            if vendor.is_none() {
                vendor = Vendor::open().map_err(|e| log::debug!("DS 模式：{e:#}")).ok();
            }
            // 手柄重连后实时扳机状态会丢，重新下发
            if reconnect && (evdev.is_some() || vendor.is_some()) {
                for (side, command) in bridge.current() {
                    send_trigger(side, command);
                }
            }
            next_open = Instant::now() + Duration::from_secs(2);
            let mut st = status.lock().unwrap();
            st.pad_connected = evdev.is_some();
            st.motion = vendor.is_some();
        }

        let mut fds = vec![libc::pollfd { fd: uhid.file.as_raw_fd(), events: libc::POLLIN, revents: 0 }];
        if let Some(e) = &evdev {
            fds.push(libc::pollfd { fd: e.file.as_raw_fd(), events: libc::POLLIN, revents: 0 });
        }
        if let Some(v) = &vendor {
            fds.push(libc::pollfd { fd: v.ctl.raw_fd(), events: libc::POLLIN, revents: 0 });
        }
        // SAFETY: fds 在调用期间有效。
        unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as _, 200) };

        let mut changed = false;
        if let Some(e) = evdev.as_mut() {
            match e.read() {
                Ok(synced) => changed |= synced,
                Err(err) => {
                    log::info!("DS 模式：{err:#}，等待重连");
                    evdev = None;
                    changed = true;
                    status.lock().unwrap().pad_connected = false;
                }
            }
        }
        if let Some(v) = vendor.as_mut() {
            let mut buf = [0u8; 64];
            let alive = loop {
                match v.ctl.read_report(&mut buf) {
                    Some(n) => {
                        if let Some(s) = decode_vendor(&buf[..n]) {
                            vendor_input = Some(VendorInput { state: s, at: Instant::now() });
                            changed = true;
                        }
                    }
                    None => break v.ctl.info.node.exists(),
                }
            };
            if !alive {
                // 节点已经消失，恢复设置也写不进去了
                if let Some(mut v) = vendor.take() {
                    v.original = None;
                }
                status.lock().unwrap().motion = false;
            }
        }
        if changed {
            compose(evdev.as_ref(), vendor_input.as_ref(), &mut state);
            uhid.send_input(&state.pack())?;
            for (side, command) in bridge.update_positions(state.l2, state.r2, options.trigger_strength) {
                send_trigger(side, command);
            }
        }

        for event in uhid.events()? {
            let data = match event {
                UhidEvent::Opened(open) => {
                    status.lock().unwrap().game_opened = open;
                    continue;
                }
                UhidEvent::Output(data) => data,
            };
            let Some(feedback) = Feedback::parse(&data) else {
                continue;
            };
            status.lock().unwrap().output_reports += 1;
            if feedback.requests_rumble() {
                let gain = |v| ds5::scale_rumble_strength(v, options.rumble_gain);
                let rumble = (gain(feedback.rumble_left), gain(feedback.rumble_right));
                if last_rumble != Some(rumble) {
                    last_rumble = Some(rumble);
                    device.send(DeviceCommand::Rumble(rumble.0, rumble.1));
                }
            }
            for (side, enabled, effect) in [
                (TriggerSide::Right, feedback.has_right_trigger(), &feedback.right_effect),
                (TriggerSide::Left, feedback.has_left_trigger(), &feedback.left_effect),
            ] {
                if !enabled {
                    continue;
                }
                if let Some(command) = bridge.apply(side, effect, options.trigger_strength) {
                    log::debug!("DS 模式：{side:?} {:02X?} -> {command:?}", effect);
                    send_trigger(side, command);
                }
            }
        }
    }
    log::info!("DS 模式：已关闭");
    device.send(DeviceCommand::Reset);
    device.send(DeviceCommand::Rumble(0, 0));
    Ok(())
}
