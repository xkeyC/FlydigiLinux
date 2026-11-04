//! 全局 DS 模式用到的虚拟 DualSense：USB 报告描述符、特性报告、输入报告打包，
//! 以及把游戏发来的输出报告转换成飞智扳机/震动命令。
//!
//! 输出报告的处理移植自 ApexSenseBridge（GPL-3.0-or-later）的 DualSenseFeedback、
//! AdaptiveTriggerTranslation、AdaptiveTriggerBridge 与 EffectStrength。描述符与特性报告取自
//! 真实 DualSense，数据来自 OpenFlydigi 的 `flydigi/ds5_usb.py`（MIT）。

pub const VENDOR_ID: u32 = 0x054C;
pub const PRODUCT_ID: u32 = 0x0CE6;
pub const NAME: &str = "Wireless Controller";

pub const REPORT_DESCRIPTOR: &[u8] = &[
    0x05, 0x01, 0x09, 0x05, 0xa1, 0x01, 0x85, 0x01, 0x09, 0x30, 0x09, 0x31, 0x09, 0x32, 0x09, 0x35, 0x09, 0x33,
    0x09, 0x34, 0x15, 0x00, 0x26, 0xff, 0x00, 0x75, 0x08, 0x95, 0x06, 0x81, 0x02, 0x06, 0x00, 0xff, 0x09, 0x20,
    0x95, 0x01, 0x81, 0x02, 0x05, 0x01, 0x09, 0x39, 0x15, 0x00, 0x25, 0x07, 0x35, 0x00, 0x46, 0x3b, 0x01, 0x65,
    0x14, 0x75, 0x04, 0x95, 0x01, 0x81, 0x42, 0x65, 0x00, 0x05, 0x09, 0x19, 0x01, 0x29, 0x0f, 0x15, 0x00, 0x25,
    0x01, 0x75, 0x01, 0x95, 0x0f, 0x81, 0x02, 0x06, 0x00, 0xff, 0x09, 0x21, 0x95, 0x0d, 0x81, 0x02, 0x06, 0x00,
    0xff, 0x09, 0x22, 0x15, 0x00, 0x26, 0xff, 0x00, 0x75, 0x08, 0x95, 0x34, 0x81, 0x02, 0x85, 0x02, 0x09, 0x23,
    0x95, 0x2f, 0x91, 0x02, 0x85, 0x05, 0x09, 0x33, 0x95, 0x28, 0xb1, 0x02, 0x85, 0x08, 0x09, 0x34, 0x95, 0x2f,
    0xb1, 0x02, 0x85, 0x09, 0x09, 0x24, 0x95, 0x13, 0xb1, 0x02, 0x85, 0x0a, 0x09, 0x25, 0x95, 0x1a, 0xb1, 0x02,
    0x85, 0x0b, 0x09, 0x41, 0x95, 0x29, 0xb1, 0x02, 0x85, 0x0c, 0x09, 0x42, 0x95, 0x29, 0xb1, 0x02, 0x85, 0x20,
    0x09, 0x26, 0x95, 0x3f, 0xb1, 0x02, 0x85, 0x21, 0x09, 0x27, 0x95, 0x04, 0xb1, 0x02, 0x85, 0x22, 0x09, 0x40,
    0x95, 0x3f, 0xb1, 0x02, 0x85, 0x80, 0x09, 0x28, 0x95, 0x3f, 0xb1, 0x02, 0x85, 0x81, 0x09, 0x29, 0x95, 0x3f,
    0xb1, 0x02, 0x85, 0x82, 0x09, 0x2a, 0x95, 0x09, 0xb1, 0x02, 0x85, 0x83, 0x09, 0x2b, 0x95, 0x3f, 0xb1, 0x02,
    0x85, 0x84, 0x09, 0x2c, 0x95, 0x3f, 0xb1, 0x02, 0x85, 0x85, 0x09, 0x2d, 0x95, 0x02, 0xb1, 0x02, 0x85, 0xa0,
    0x09, 0x2e, 0x95, 0x01, 0xb1, 0x02, 0x85, 0xe0, 0x09, 0x2f, 0x95, 0x3f, 0xb1, 0x02, 0x85, 0xf0, 0x09, 0x30,
    0x95, 0x3f, 0xb1, 0x02, 0x85, 0xf1, 0x09, 0x31, 0x95, 0x3f, 0xb1, 0x02, 0x85, 0xf2, 0x09, 0x32, 0x95, 0x0f,
    0xb1, 0x02, 0x85, 0xf4, 0x09, 0x35, 0x95, 0x3f, 0xb1, 0x02, 0x85, 0xf5, 0x09, 0x36, 0x95, 0x03, 0xb1, 0x02,
    0xc0,
];

/// 驱动（hid-playstation、SDL）会读的特性报告；内容不含报告 ID，回复时补在最前面。
pub fn feature_report(id: u8) -> Option<&'static [u8]> {
    Some(match id {
        0x05 => &[
            0x00, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x92, 0x22, 0x6e, 0xdd, 0x8a, 0x22, 0x8a, 0xdd, 0x8a, 0x22, 0x76,
            0xdd, 0x1c, 0x02, 0x1c, 0x02, 0x03, 0x20, 0xf8, 0xdf, 0xe5, 0x1f, 0xe7, 0xdf, 0xf2, 0x1f, 0xe4, 0xdf,
            0x0a, 0x00, 0x00, 0x00, 0x00, 0x00,
        ],
        0x09 => &[
            0x74, 0xe7, 0xd6, 0x3a, 0x53, 0x35, 0x08, 0x25, 0x00, 0x1e, 0x00, 0xee, 0x74, 0xd0, 0xbc, 0x00, 0x00,
            0x00, 0x00,
        ],
        0x0B => &[
            0x74, 0xe7, 0xd6, 0x3a, 0x53, 0x35, 0x08, 0x25, 0x00, 0x01, 0x1e, 0x00, 0xee, 0x74, 0xd0, 0xbc, 0x00,
            0x1e, 0x00, 0xee, 0x74, 0xd0, 0xbc, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ],
        0x20 => &[
            0x4a, 0x75, 0x6c, 0x20, 0x20, 0x34, 0x20, 0x32, 0x30, 0x32, 0x35, 0x31, 0x30, 0x3a, 0x33, 0x38, 0x3a,
            0x34, 0x30, 0x03, 0x00, 0x0b, 0x00, 0x11, 0x08, 0x00, 0x00, 0x2a, 0x00, 0x10, 0x01, 0x01, 0x48, 0x02,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x30, 0x06, 0x00, 0x00, 0x01, 0x00, 0x03, 0x00,
            0x10, 0x10, 0x03, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ],
        _ => return None,
    })
}

pub mod button {
    // buttons0（高 4 位，低 4 位是方向键）
    pub const SQUARE: u8 = 0x10;
    pub const CROSS: u8 = 0x20;
    pub const CIRCLE: u8 = 0x40;
    pub const TRIANGLE: u8 = 0x80;
    // buttons1
    pub const L1: u8 = 0x01;
    pub const R1: u8 = 0x02;
    pub const L2: u8 = 0x04;
    pub const R2: u8 = 0x08;
    pub const CREATE: u8 = 0x10;
    pub const OPTIONS: u8 = 0x20;
    pub const L3: u8 = 0x40;
    pub const R3: u8 = 0x80;
    // buttons2
    pub const PS: u8 = 0x01;
    pub const TOUCHPAD: u8 = 0x02;
}

pub const HAT_NEUTRAL: u8 = 8;

/// 方向键（-1/0/1）编码成 DualSense 的 hat 值。
pub fn hat(x: i32, y: i32) -> u8 {
    match (x.signum(), y.signum()) {
        (0, -1) => 0,
        (1, -1) => 1,
        (1, 0) => 2,
        (1, 1) => 3,
        (0, 1) => 4,
        (-1, 1) => 5,
        (-1, 0) => 6,
        (-1, -1) => 7,
        _ => HAT_NEUTRAL,
    }
}

/// 虚拟 DualSense 的输入状态，USB 输入报告 0x01，64 字节。
#[derive(Clone, Debug)]
pub struct InputState {
    pub lx: u8,
    pub ly: u8,
    pub rx: u8,
    pub ry: u8,
    pub l2: u8,
    pub r2: u8,
    pub hat: u8,
    pub buttons: [u8; 3],
    pub gyro: [i16; 3],
    pub accel: [i16; 3],
    seq: u8,
    timestamp: u32,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            lx: 0x80,
            ly: 0x80,
            rx: 0x80,
            ry: 0x80,
            l2: 0,
            r2: 0,
            hat: HAT_NEUTRAL,
            buttons: [0; 3],
            gyro: [0; 3],
            accel: [0; 3],
            seq: 0,
            timestamp: 0,
        }
    }
}

impl InputState {
    pub fn set(&mut self, group: usize, mask: u8, pressed: bool) {
        if pressed {
            self.buttons[group] |= mask;
        } else {
            self.buttons[group] &= !mask;
        }
    }

    /// 手柄掉线时松开所有输入，避免游戏里卡着某个键。
    pub fn release(&mut self) {
        *self = Self { seq: self.seq, timestamp: self.timestamp, ..Self::default() };
    }

    pub fn pack(&mut self) -> [u8; 64] {
        let mut r = [0u8; 64];
        r[0] = 0x01;
        r[1..7].copy_from_slice(&[self.lx, self.ly, self.rx, self.ry, self.l2, self.r2]);
        r[7] = self.seq;
        r[8] = (self.hat & 0x0F) | (self.buttons[0] & 0xF0);
        r[9] = self.buttons[1];
        r[10] = self.buttons[2];
        for (i, v) in self.gyro.iter().chain(&self.accel).enumerate() {
            r[16 + i * 2..18 + i * 2].copy_from_slice(&v.to_le_bytes());
        }
        r[28..32].copy_from_slice(&self.timestamp.to_le_bytes());
        // 触摸点都标记为未按下
        r[33] = 0x80;
        r[37] = 0x80;
        // 电量满、未充电
        r[53] = 0x0A | (0x02 << 4);
        r[54] = 0x0C;
        self.seq = self.seq.wrapping_add(1);
        self.timestamp = self.timestamp.wrapping_add(1000);
        r
    }
}


use flydigi_proto::TriggerSide;

/// 飞智扳机命令：(mode, 5 个参数)，mode 为 AdapterTriggerType。
pub type TriggerCommand = (u8, [u8; 5]);

const MODE_NORMAL: u8 = 0;
const MODE_RACE: u8 = 1;
const MODE_RECOIL_RATTLE: u8 = 2;
const MODE_SNIPER_BREAK: u8 = 3;

/// USB 输出报告（`data[0] == 0x02`）里与手柄反馈相关的字段。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Feedback {
    pub enable1: u8,
    pub enable2: u8,
    pub rumble_right: u8,
    pub rumble_left: u8,
    pub enable3: u8,
    pub right_effect: [u8; 11],
    pub left_effect: [u8; 11],
    pub lightbar: Option<[u8; 3]>,
}

impl Feedback {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 33 || data[0] != 0x02 {
            return None;
        }
        let at = |i: usize| data.get(i).copied().unwrap_or(0);
        let mut f = Feedback {
            enable1: at(1),
            enable2: at(2),
            rumble_right: at(3),
            rumble_left: at(4),
            enable3: at(39),
            ..Default::default()
        };
        f.right_effect.copy_from_slice(&data[11..22]);
        f.left_effect.copy_from_slice(&data[22..33]);
        if data.len() >= 48 && f.enable2 & 0x04 != 0 {
            f.lightbar = Some([data[45], data[46], data[47]]);
        }
        Some(f)
    }

    /// 兼容震动：flag0 bit0 或 flag2 bit2。
    pub fn requests_rumble(&self) -> bool {
        self.enable1 & 0x01 != 0 || self.enable3 & 0x04 != 0
    }

    pub fn has_right_trigger(&self) -> bool {
        self.enable1 & 0x04 != 0
    }

    pub fn has_left_trigger(&self) -> bool {
        self.enable1 & 0x08 != 0
    }
}

// ------------------------------------------------------------ 自适应扳机翻译

fn stroke_for_zone(zone: u32) -> u8 {
    const MAXIMUM_STROKE: u32 = 192;
    ((zone * MAXIMUM_STROKE + 5) / 10) as u8
}

fn limited_force(strength: u8) -> Option<u8> {
    const LIMITED_MAXIMUM: u32 = 10;
    const NATIVE_FORCE_CEILING: u32 = 64;
    let s = strength as u32;
    (s <= LIMITED_MAXIMUM).then(|| ((s * NATIVE_FORCE_CEILING + LIMITED_MAXIMUM / 2) / LIMITED_MAXIMUM) as u8)
}

#[derive(Clone, Copy)]
struct ZoneRange {
    first: u32,
    last: u32,
}

fn scan_zones(mask: u32) -> ZoneRange {
    let mut range = ZoneRange { first: 10, last: 0 };
    for zone in 0..10 {
        if mask & (1 << zone) != 0 {
            range.first = range.first.min(zone);
            range.last = zone;
        }
    }
    range
}

fn make_break(mut zones: ZoneRange, strength: u32) -> TriggerCommand {
    if zones.first == zones.last {
        zones.last = (zones.first + 1).min(9);
        zones.first = zones.last - 1;
    }
    let start = stroke_for_zone(zones.first);
    let length = stroke_for_zone(zones.last) - start;
    (MODE_SNIPER_BREAK, [start, length, (strength * 8) as u8, 0, 0])
}

fn make_bow(zones: ZoneRange, draw_strength: u32, snap_force: u32) -> TriggerCommand {
    const LAST_ZONE: u32 = 9;
    const STRONG_SNAP: u32 = 7;
    let start = stroke_for_zone(zones.first);
    let resistance = (draw_strength * 8) as u8;
    if zones.first >= zones.last || zones.last >= LAST_ZONE || snap_force < STRONG_SNAP {
        return (MODE_RACE, [start, resistance, 0, 0, 0]);
    }
    let length = stroke_for_zone(zones.last) - start;
    (MODE_SNIPER_BREAK, [start, length, resistance, 0, 0])
}

/// DualSense 原生效果（0x21–0x27，10 个区段的位掩码）。
fn translate_native(effect: &[u8; 11]) -> Option<TriggerCommand> {
    let mask = effect[1] as u32 | (effect[2] as u32) << 8;
    if mask & !0x03FF != 0 {
        return None;
    }
    if mask == 0 {
        return Some((MODE_NORMAL, [0; 5]));
    }
    let zones = scan_zones(mask);
    let start = stroke_for_zone(zones.first);
    match effect[0] {
        0x22 => return Some(make_bow(zones, (effect[3] & 7) as u32 + 1, ((effect[3] >> 3) & 7) as u32 + 1)),
        0x25 => return Some(make_break(zones, (effect[3] & 7) as u32 + 1)),
        0x23 => {
            return Some(if effect[4] == 0 {
                (MODE_NORMAL, [0; 5])
            } else {
                (MODE_RECOIL_RATTLE, [start, 1, 60, effect[4], 0])
            });
        }
        0x27 => {
            if effect[4] == 0 {
                return Some((MODE_NORMAL, [0; 5]));
            }
            let average = ((effect[3] & 7) as u32 + ((effect[3] >> 3) & 7) as u32) / 2 + 1;
            return Some((MODE_RECOIL_RATTLE, [start, 1, (average * 10) as u8, effect[4], 0]));
        }
        _ => {}
    }
    let packed = u32::from_le_bytes([effect[3], effect[4], effect[5], effect[6]]);
    let mut strengths = [0u32; 10];
    let mut peak = 0;
    for zone in 0..10 {
        if mask & (1 << zone) != 0 {
            strengths[zone] = ((packed >> (zone * 3)) & 7) + 1;
            peak = peak.max(strengths[zone]);
        }
    }
    if effect[0] == 0x21 {
        return Some((MODE_RACE, [start, (peak * 8) as u8, 0, 0, 0]));
    }
    let frequency = effect[9];
    if frequency == 0 {
        return Some((MODE_NORMAL, [0; 5]));
    }
    // 从强度达到峰值一半的区段开始取平均，而不是只看峰值
    let mut first = zones.first as usize;
    while strengths[first] * 2 < peak {
        first += 1;
    }
    let (mut sum, mut count) = (0, 0);
    for s in strengths[first..].iter().filter(|s| **s != 0) {
        sum += s;
        count += 1;
    }
    let mut amplitude = (sum * 10 + count / 2) / count;
    if frequency < 10 {
        amplitude /= 2;
    }
    Some((MODE_RECOIL_RATTLE, [stroke_for_zone(first as u32), 1, amplitude.max(1) as u8, frequency, 0]))
}

/// DualSense 扳机效果块（11 字节，`[0]` 为效果类型）翻译成飞智命令；不支持的返回 None。
pub fn translate_adaptive_trigger(effect: &[u8; 11]) -> Option<TriggerCommand> {
    let t = effect[0];
    match t {
        0x01 => Some((MODE_RACE, [effect[1], effect[2], 0, 0, 0])),
        0x11 => {
            let force = limited_force(effect[2])?;
            Some(if force == 0 { (MODE_NORMAL, [0; 5]) } else { (MODE_RACE, [effect[1], force, 0, 0, 0]) })
        }
        0x02 | 0x12 => {
            if effect[2] < effect[1] {
                return None;
            }
            let length = (effect[2] - effect[1]).max(1);
            if t == 0x02 {
                return Some((MODE_SNIPER_BREAK, [effect[1], length, effect[3], 0, 0]));
            }
            let force = limited_force(effect[3])?;
            Some(if force == 0 { (MODE_NORMAL, [0; 5]) } else { (MODE_SNIPER_BREAK, [effect[1], length, force, 0, 0]) })
        }
        0x05 => Some((MODE_NORMAL, [0; 5])),
        0x06 => Some((MODE_RECOIL_RATTLE, [effect[3], effect[2], effect[2], effect[1], 0])),
        0x21..=0x23 | 0x25..=0x27 => translate_native(effect),
        _ => None,
    }
}

/// 弓（0x22，强回弹）：先给拉弓阻力，扳机越过断点后切成断裂效果，松回去再恢复。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BowEmulation {
    pub draw: TriggerCommand,
    pub rupture: TriggerCommand,
    pub rupture_axis: u8,
    pub rearm_axis: u8,
}

fn axis_for_stroke(stroke: u32) -> u32 {
    (stroke * 255 / 192).min(255)
}

pub fn describe_bow_emulation(effect: &[u8; 11]) -> Option<BowEmulation> {
    const LAST_ZONE: u32 = 9;
    const STRONG_SNAP: u32 = 7;
    const RUPTURE_LENGTH: u8 = 40;
    const RUPTURE_AXIS_PERCENT: u32 = 85;
    const REARM_MARGIN: u32 = 10;
    if effect[0] != 0x22 {
        return None;
    }
    let mask = effect[1] as u32 | (effect[2] as u32) << 8;
    if mask == 0 || mask & !0x03FF != 0 {
        return None;
    }
    let zones = scan_zones(mask);
    let draw_strength = (effect[3] & 7) as u32 + 1;
    let snap_force = ((effect[3] >> 3) & 7) as u32 + 1;
    if zones.first >= zones.last || zones.last >= LAST_ZONE || snap_force < STRONG_SNAP {
        return None;
    }
    let start = stroke_for_zone(zones.first);
    let end = stroke_for_zone(zones.last);
    let resistance = (draw_strength * 8) as u8;
    let start_axis = axis_for_stroke(start as u32);
    Some(BowEmulation {
        draw: (MODE_RACE, [start, resistance, 0, 0, 0]),
        rupture: (MODE_SNIPER_BREAK, [end, RUPTURE_LENGTH, resistance, 0, 0]),
        rupture_axis: (axis_for_stroke(end as u32) * RUPTURE_AXIS_PERCENT / 100) as u8,
        rearm_axis: if start_axis > 2 * REARM_MARGIN { start_axis - REARM_MARGIN } else { REARM_MARGIN } as u8,
    })
}

/// 只缩放力度字段，行程、区段、频率不变；缩到 0 就当普通模式。
pub fn scale_trigger_strength(command: TriggerCommand, percent: u32) -> TriggerCommand {
    let (mode, mut p) = command;
    if percent >= 100 || mode == MODE_NORMAL {
        return command;
    }
    if percent == 0 {
        return (MODE_NORMAL, [0; 5]);
    }
    let scale = |v: u8| (v as u32 * percent / 100) as u8;
    let index = match mode {
        MODE_RACE => 1,
        MODE_SNIPER_BREAK | MODE_RECOIL_RATTLE => 2,
        _ => return command,
    };
    p[index] = scale(p[index]);
    if p[index] == 0 { (MODE_NORMAL, [0; 5]) } else { (mode, p) }
}

/// 握把马达增益：100% 以内线性，以上用饱和曲线抬高中低档而不削顶。
pub fn scale_rumble_strength(value: u8, percent: u32) -> u8 {
    let capped = percent.min(200);
    if capped <= 100 {
        return (value as u32 * capped / 100) as u8;
    }
    let remaining = 1.0 - value as f64 / 255.0;
    let exponent = 2.0 * capped as f64 / 100.0 - 1.0;
    let lifted = (255.0 * (1.0 - remaining.powf(exponent))).round() as i64;
    lifted.clamp(value as i64, 255) as u8
}

/// 一侧扳机的翻译状态（AdaptiveTriggerBridge）。
#[derive(Default)]
pub struct TriggerBridge {
    last: [Option<TriggerCommand>; 2],
    bow: [Option<([u8; 11], BowEmulation, bool)>; 2],
}

fn index(side: TriggerSide) -> usize {
    side as usize - 1
}

impl TriggerBridge {
    /// 处理一块效果，返回需要发给手柄的命令。
    pub fn apply(&mut self, side: TriggerSide, effect: &[u8; 11], strength: u32) -> Option<TriggerCommand> {
        // 和真 DualSense 一样保持当前效果：有的游戏会在有效效果之间夹空块，遇空就释放会让扳机来回抖
        if effect[0] == 0 {
            return None;
        }
        let mut command = translate_adaptive_trigger(effect)?;
        let i = index(side);
        let bow = describe_bow_emulation(effect);
        match (&bow, &self.bow[i]) {
            (Some(_), Some((armed, _, _))) if armed == effect => return None,
            (Some(b), _) => {
                self.bow[i] = Some((*effect, b.clone(), false));
                command = b.draw;
            }
            (None, _) => self.bow[i] = None,
        }
        let command = scale_trigger_strength(command, strength);
        if self.last[i] == Some(command) {
            return None;
        }
        self.last[i] = Some(command);
        Some(command)
    }

    /// 弓效果按扳机实际位置在「拉弓」和「断裂」之间切换。
    pub fn update_positions(&mut self, left: u8, right: u8, strength: u32) -> Vec<(TriggerSide, TriggerCommand)> {
        let mut out = Vec::new();
        for (side, position) in [(TriggerSide::Left, left), (TriggerSide::Right, right)] {
            let i = index(side);
            let Some((_, bow, ruptured)) = self.bow[i].as_mut() else {
                continue;
            };
            let command = if !*ruptured && position >= bow.rupture_axis {
                *ruptured = true;
                bow.rupture
            } else if *ruptured && position <= bow.rearm_axis {
                *ruptured = false;
                bow.draw
            } else {
                continue;
            };
            let command = scale_trigger_strength(command, strength);
            self.last[i] = Some(command);
            out.push((side, command));
        }
        out
    }

    /// 手柄重连后需要重发的当前效果。
    pub fn current(&self) -> Vec<(TriggerSide, TriggerCommand)> {
        [TriggerSide::Left, TriggerSide::Right]
            .into_iter()
            .filter_map(|s| self.last[index(s)].map(|c| (s, c)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_size_matches_hardware() {
        assert_eq!(REPORT_DESCRIPTOR.len(), 289);
    }

    #[test]
    fn feedback_layout() {
        let mut r = [0u8; 48];
        r[0] = 0x02;
        r[1] = 0x0D;
        r[3] = 200;
        r[4] = 100;
        r[11] = 0x01;
        r[12] = 40;
        r[13] = 180;
        r[22] = 0x05;
        let f = Feedback::parse(&r).unwrap();
        assert!(f.requests_rumble() && f.has_right_trigger() && f.has_left_trigger());
        assert_eq!((f.rumble_left, f.rumble_right), (100, 200));
        assert_eq!(translate_adaptive_trigger(&f.right_effect), Some((1, [40, 180, 0, 0, 0])));
        assert_eq!(translate_adaptive_trigger(&f.left_effect), Some((0, [0; 5])));
    }

    #[test]
    fn native_feedback_zones() {
        // 0x21 Feedback：区段 3..9，强度 4
        let mut e = [0u8; 11];
        e[0] = 0x21;
        e[1] = 0xF8;
        e[2] = 0x03;
        e[3] = 0xDB;
        e[4] = 0xB6;
        e[5] = 0x6D;
        e[6] = 0x1B;
        assert_eq!(translate_adaptive_trigger(&e), Some((1, [58, 32, 0, 0, 0])));
    }

    #[test]
    fn neutral_blocks_keep_effect() {
        let mut b = TriggerBridge::default();
        let mut e = [0u8; 11];
        e[0] = 0x01;
        e[1] = 20;
        e[2] = 200;
        assert!(b.apply(TriggerSide::Right, &e, 100).is_some());
        assert!(b.apply(TriggerSide::Right, &[0; 11], 100).is_none());
        assert!(b.apply(TriggerSide::Right, &e, 100).is_none());
    }

    #[test]
    fn strength_scaling() {
        assert_eq!(scale_trigger_strength((1, [10, 200, 0, 0, 0]), 50), (1, [10, 100, 0, 0, 0]));
        assert_eq!(scale_trigger_strength((3, [10, 20, 1, 0, 0]), 50), (0, [0; 5]));
        assert_eq!(scale_rumble_strength(100, 100), 100);
        assert!(scale_rumble_strength(100, 200) > 100);
    }
}
