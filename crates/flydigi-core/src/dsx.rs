//! 官方 mod 发往 UDP 7878 的指令（DSX 格式的 JSON）。
//!
//! `{"instructions":[{"type":1,"parameters":[0, side, 19, mode, p…]}, …]}`，
//! 按 ControllerBusinessService.OnTriggerCommandReceived 的方式转换：
//! 取 `parameters[1]` 为左右，`parameters[3..]` 依次作为 mode 和参数，最多 7 字节。

use flydigi_proto::{ForceTrigger, TriggerSide};
use serde_json::Value;

const TRIGGER_UPDATE: i64 = 1;

/// 对齐 C# 的 `byte.TryParse(obj.ToString())`：只有 0..=255 的整数有效，其余一律当 0。
fn to_byte(v: &Value) -> u8 {
    let parsed = match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    };
    match parsed {
        Some(f) if f.fract() == 0.0 && (0.0..=255.0).contains(&f) => f as u8,
        _ => 0,
    }
}

/// 解析一个 UDP 包，返回要发给手柄的效果（每侧最多一个，后出现的覆盖先出现的）。
pub fn parse_packet(data: &[u8]) -> Option<Vec<ForceTrigger>> {
    let packet: Value = serde_json::from_slice(data).ok()?;
    let mut left = None;
    let mut right = None;
    for ins in packet.get("instructions")?.as_array()? {
        if ins.get("type").and_then(Value::as_i64) != Some(TRIGGER_UPDATE) {
            continue;
        }
        let Some(params) = ins.get("parameters").and_then(Value::as_array) else {
            continue;
        };
        if params.len() < 3 {
            continue;
        }
        if params[3..].iter().any(Value::is_null) {
            // 官方遇到 null 参数直接放弃整包
            return None;
        }
        let mut bytes = [0u8; 7];
        bytes[0] = to_byte(&params[1]);
        for (slot, v) in bytes[1..].iter_mut().zip(&params[3..]) {
            *slot = to_byte(v);
        }
        let side = TriggerSide::from_u8(bytes[0]);
        let effect = ForceTrigger::common(side, bytes[1], &bytes[2..]);
        match side {
            TriggerSide::Left => left = Some(effect),
            TriggerSide::Right => right = Some(effect),
        }
    }
    Some(left.into_iter().chain(right).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forza_dualsense_packet() {
        let data = br#"{"instructions":[{"type":1,"parameters":[0,2,19,2,50,1,200,1]},{"type":0,"parameters":null}]}"#;
        let effects = parse_packet(data).unwrap();
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].side, TriggerSide::Right);
        assert_eq!(effects[0].mode, 2);
        assert_eq!(&effects[0].params[..4], &[50, 1, 200, 1]);
    }

    #[test]
    fn invalid_numbers_become_zero() {
        let data = br#"{"instructions":[{"type":1,"parameters":[0,"1",19,3,1.5,-2,300,"7"]}]}"#;
        let effects = parse_packet(data).unwrap();
        assert_eq!(effects[0].side, TriggerSide::Left);
        assert_eq!(effects[0].params, vec![0, 0, 0, 7, 0]);
    }
}
