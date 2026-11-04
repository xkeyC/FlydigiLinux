//! 力反馈扳机效果（命令 81）和「扳机跟随握把震动」绑定（命令 82）。

use crate::{build_report, cmd};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TriggerSide {
    Left = 1,
    Right = 2,
}

impl TriggerSide {
    pub fn from_u8(v: u8) -> Self {
        if v == 2 { Self::Right } else { Self::Left }
    }
}

/// `AdapterTriggerType`。注意 Space Station 界面把 2/3 的名字对调了：
/// 模式 2 界面叫「机枪」（震颤），模式 3 界面叫「狙击」（阻力后突破）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriggerMode {
    Normal = 0,
    Race = 1,
    Sniper = 2,
    Recoil = 3,
    Lock = 4,
    Vibration = 5,
}

impl TriggerMode {
    pub const ALL: [TriggerMode; 6] = [
        Self::Normal,
        Self::Race,
        Self::Sniper,
        Self::Recoil,
        Self::Lock,
        Self::Vibration,
    ];

    pub fn from_u8(v: u8) -> Option<Self> {
        Self::ALL.get(v as usize).copied()
    }

    /// Space Station 界面上的名字。
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "普通",
            Self::Race => "赛车",
            Self::Sniper => "机枪",
            Self::Recoil => "狙击",
            Self::Lock => "锁定",
            Self::Vibration => "震动",
        }
    }
}

/// 一侧扳机的效果：`side, mode, params…`，总长最多 7 字节。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForceTrigger {
    pub side: TriggerSide,
    pub mode: u8,
    pub params: Vec<u8>,
}

impl ForceTrigger {
    pub fn normal(side: TriggerSide) -> Self {
        Self { side, mode: TriggerMode::Normal as u8, params: vec![] }
    }

    /// `ForceTriggerConfigCommon`：Space Station 实时路径（DSX/适配游戏）用的构造方式，
    /// 包含它对 Race 的改写——行程为 0 且 matchStroke=1 时把 matchStroke 清零。
    pub fn common(side: TriggerSide, mode: u8, params: &[u8]) -> Self {
        let mut params: Vec<u8> = params.iter().copied().take(5).collect();
        if mode == TriggerMode::Race as u8
            && params.first() == Some(&0)
            && params.get(2) == Some(&1)
        {
            params[2] = 0;
        }
        Self { side, mode, params }
    }

    /// 命令 81 的报告。`apply=false` 对应 Space Station 的 onlyPreview：只改实时状态，不写入手柄存储。
    pub fn report(&self, apply: bool) -> [u8; 32] {
        let mut payload = vec![10, apply as u8, self.side as u8, self.mode];
        payload.extend_from_slice(&self.params);
        build_report(cmd::FORCE_TRIGGER, &payload)
    }
}

/// 命令 82：让扳机马达跟随握把震动（`ForceTriggerConfigSyncWithGrip`）。
/// `bind_type` 为 0 表示解除绑定，适配游戏里一律是 2。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GripBind {
    pub side: TriggerSide,
    pub bind_type: u8,
    pub filter: u8,
    pub scale: u8,
    /// stroke, pressureLevel, strength, frequency
    pub params: [u8; 4],
}

impl GripBind {
    pub fn unbind(side: TriggerSide) -> Self {
        Self { side, bind_type: 0, filter: 0, scale: 0, params: [0; 4] }
    }

    pub fn report(&self) -> [u8; 32] {
        let mut payload = vec![11, self.side as u8, self.bind_type, self.filter, self.scale];
        payload.extend_from_slice(&self.params);
        build_report(cmd::FORCE_TRIGGER_SYNC_GRIP, &payload)
    }
}

/// 命令 18：握把马达，左（低频）右（高频）各 0..255。
pub fn rumble_report(left: u8, right: u8) -> [u8; 32] {
    build_report(cmd::RUMBLE, &[6, left, right])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn race_rewrite_matches_space_station() {
        let t = ForceTrigger::common(TriggerSide::Right, 1, &[0, 1, 1]);
        assert_eq!(t.params, vec![0, 1, 0]);
        let t = ForceTrigger::common(TriggerSide::Right, 1, &[10, 1, 1]);
        assert_eq!(t.params, vec![10, 1, 1]);
    }

    #[test]
    fn force_trigger_wire_format() {
        let r = ForceTrigger::common(TriggerSide::Right, 4, &[120, 255, 1]).report(false);
        assert_eq!(&r[..11], &[3, 0x5A, 0xA5, 0x51, 10, 0, 2, 4, 120, 255, 1]);
    }

    #[test]
    fn grip_bind_wire_format() {
        let b = GripBind { side: TriggerSide::Left, bind_type: 2, filter: 1, scale: 10, params: [0, 1, 100, 15] };
        assert_eq!(&b.report()[..13], &[3, 0x5A, 0xA5, 0x52, 11, 1, 2, 1, 10, 0, 1, 100, 15]);
    }
}
