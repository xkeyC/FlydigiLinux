//! 飞智官方「扳机适配」游戏列表（`/pc/adapter_trigger/list`，公开接口，无需登录）。

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize};

use crate::paths;

pub const LIST_URL: &str = "https://api.flydigi.com/pc/adapter_trigger/list?version=0";

/// 接口里同一字段时而是数字、时而是字符串或 null，统一按宽松方式解析。
fn lenient_i64<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    Ok(match serde_json::Value::deserialize(d)? {
        serde_json::Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).unwrap_or(0),
        serde_json::Value::String(s) => s.trim().parse().unwrap_or(0),
        serde_json::Value::Bool(b) => b as i64,
        _ => 0,
    })
}

fn lenient_string<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(match serde_json::Value::deserialize(d)? {
        serde_json::Value::String(s) => s,
        serde_json::Value::Null => String::new(),
        v => v.to_string(),
    })
}

fn lenient_strings<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    Ok(match serde_json::Value::deserialize(d)? {
        serde_json::Value::Array(a) => a
            .into_iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    })
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Game {
    #[serde(deserialize_with = "lenient_i64")]
    pub id: i64,
    #[serde(deserialize_with = "lenient_string")]
    pub game_name: String,
    #[serde(deserialize_with = "lenient_string")]
    pub en_game_name: String,
    #[serde(deserialize_with = "lenient_string")]
    pub platform: String,
    #[serde(rename = "platform_game_ids", deserialize_with = "lenient_string")]
    pub platform_game_ids: String,
    #[serde(deserialize_with = "lenient_string")]
    pub image_path: String,
    #[serde(deserialize_with = "lenient_string")]
    pub process_game_name: String,
    #[serde(deserialize_with = "lenient_strings")]
    pub process_game_names: Vec<String>,
    #[serde(deserialize_with = "lenient_string")]
    pub exe_name: String,
    #[serde(deserialize_with = "lenient_i64")]
    pub mod_start_type: i64,
    #[serde(deserialize_with = "lenient_string")]
    pub mod_path: String,
    #[serde(deserialize_with = "lenient_string")]
    pub mod_name: String,
    #[serde(deserialize_with = "lenient_string")]
    pub process_game_mod_name: String,
    #[serde(deserialize_with = "lenient_string")]
    pub mod_down_load_url: String,
    #[serde(deserialize_with = "lenient_string")]
    pub version: String,
    #[serde(deserialize_with = "lenient_string")]
    pub update_time_str: String,
    /// 官方的扳机效果介绍（纯文本，按行排版）。
    #[serde(deserialize_with = "lenient_string")]
    pub scenes: String,
    /// 官方注意事项：文字夹着 `<img src='…' />`。
    #[serde(deserialize_with = "lenient_string")]
    pub notice_pic_url: String,
    #[serde(deserialize_with = "lenient_string")]
    pub en_notice_pic_url: String,
    /// 更新说明。
    #[serde(deserialize_with = "lenient_string")]
    pub update: String,
    #[serde(deserialize_with = "lenient_i64")]
    pub is_mod: i64,
    #[serde(rename = "isPS5", deserialize_with = "lenient_i64")]
    pub is_ps5: i64,
    #[serde(deserialize_with = "lenient_i64")]
    pub is_vibration: i64,
    #[serde(deserialize_with = "lenient_i64")]
    pub is_racing_telemetry: i64,
    #[serde(deserialize_with = "lenient_i64")]
    pub vib_type: i64,
    #[serde(deserialize_with = "lenient_i64")]
    pub vib_filter: i64,
    #[serde(deserialize_with = "lenient_i64")]
    pub vib_filter_right: i64,
    #[serde(deserialize_with = "lenient_string")]
    pub vib_params: String,
    #[serde(deserialize_with = "lenient_string")]
    pub vib_params_right: String,
    #[serde(deserialize_with = "lenient_i64")]
    pub pwm_scal: i64,
    #[serde(deserialize_with = "lenient_i64")]
    pub pwm_scal_right: i64,
}

/// 一个游戏在 Linux 上走哪条扳机路线。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Route {
    /// 命令 82：扳机跟随游戏本身的握把震动，不需要 mod。
    Vibration,
    /// 官方 XGameMonitor：按 JSON 配置读游戏内存。用 dotnet 运行，XHelper.dll 换成 Linux 版。
    Monitor,
    /// 官方 ForzaDualSense：Forza Data Out 遥测转扳机指令，用 dotnet 运行。
    Telemetry,
    /// 其他外置 mod 程序（如 F1 系列）。.NET 的用 dotnet，原生 Windows 程序用 Wine。
    ExternalExe,
    /// 放进游戏目录的 mod，在游戏进程里经 UDP 7878 发指令。
    InGameMod,
    /// 只有 DualSense 模式（虚拟 PS5 手柄），暂未实现。
    DsOnly,
}

impl Route {
    pub fn label(self) -> &'static str {
        match self {
            Route::Vibration => "震动绑定",
            Route::Monitor => "内存监控",
            Route::Telemetry => "遥测",
            Route::ExternalExe => "外置程序",
            Route::InGameMod => "游戏内 mod",
            Route::DsOnly => "仅 DS 模式",
        }
    }

    /// 需要先下载官方 mod 包才能启用。
    pub fn needs_download(self) -> bool {
        matches!(self, Route::Monitor | Route::Telemetry | Route::ExternalExe | Route::InGameMod)
    }

    pub fn supported(self) -> bool {
        self != Route::DsOnly
    }
}

impl Game {
    pub fn display_name(&self) -> &str {
        if self.game_name.is_empty() { &self.en_game_name } else { &self.game_name }
    }

    pub fn route(&self) -> Route {
        let has_url = !self.mod_down_load_url.trim().is_empty();
        if self.is_mod != 0 && self.mod_start_type == 1 && !self.mod_name.is_empty() && has_url {
            return match self.mod_name.to_ascii_lowercase().as_str() {
                "xgamemonitor.exe" => Route::Monitor,
                "forzadualsense.exe" => Route::Telemetry,
                _ => Route::ExternalExe,
            };
        }
        if self.is_mod != 0 && self.mod_start_type == 0 && has_url {
            return Route::InGameMod;
        }
        if self.is_vibration != 0 {
            return Route::Vibration;
        }
        Route::DsOnly
    }

    /// 官方 CheckGame 里，外置 mod 分支优先于震动分支；游戏内 mod 的游戏仍可同时开震动绑定（如辐射 4）。
    pub fn applies_vibration(&self) -> bool {
        self.is_vibration != 0
            && !matches!(self.route(), Route::Monitor | Route::Telemetry | Route::ExternalExe)
    }

    /// 所有进程名，小写、不带 `.exe`。
    pub fn process_names(&self) -> Vec<String> {
        let mut names: Vec<String> = std::iter::once(&self.process_game_name)
            .chain(self.process_game_names.iter())
            .map(|n| n.trim().trim_end_matches(".exe").to_lowercase())
            .filter(|n| !n.is_empty())
            .collect();
        names.dedup();
        names
    }

    /// 官方注意事项拆成文字段和图片地址。
    pub fn notice(&self, english: bool) -> Vec<NoticePart> {
        let raw = if english && !self.en_notice_pic_url.trim().is_empty() { &self.en_notice_pic_url } else { &self.notice_pic_url };
        parse_notice(raw)
    }

    pub fn steam_app_ids(&self) -> Vec<u32> {
        self.platform_game_ids.split(',').filter_map(|s| s.trim().parse().ok()).collect()
    }

    /// 外置 mod 在解压目录里的位置，例如 `mods/forza_trigger`。
    pub fn mod_rel_dir(&self) -> PathBuf {
        PathBuf::from(self.mod_path.replace('\\', "/").trim_matches('/'))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoticePart {
    Text(String),
    Image(String),
}

/// 把 `文字<img src='url' />文字` 拆开；其他 HTML 标签直接去掉。
pub fn parse_notice(raw: &str) -> Vec<NoticePart> {
    let mut parts = Vec::new();
    let mut rest = raw;
    let push_text = |parts: &mut Vec<NoticePart>, text: &str| {
        let mut clean = String::new();
        let mut in_tag = false;
        for c in text.chars() {
            match c {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => clean.push(c),
                _ => {}
            }
        }
        let clean = clean.lines().map(str::trim_end).collect::<Vec<_>>().join("\n");
        let clean = clean.trim();
        if !clean.is_empty() {
            parts.push(NoticePart::Text(clean.to_owned()));
        }
    };
    while let Some(start) = rest.find("<img") {
        push_text(&mut parts, &rest[..start]);
        let tag_end = rest[start..].find('>').map_or(rest.len(), |i| start + i + 1);
        let tag = &rest[start..tag_end];
        if let Some(src) = tag.find("src=") {
            let value = &tag[src + 4..];
            let quote = value.chars().next().unwrap_or('"');
            let url: String = value[1..].chars().take_while(|c| *c != quote).collect();
            if url.starts_with("http") {
                parts.push(NoticePart::Image(url));
            }
        }
        rest = &rest[tag_end..];
    }
    push_text(&mut parts, rest);
    parts
}

pub fn fetch() -> Result<Vec<Game>> {
    let body = ureq::get(LIST_URL)
        .call()
        .context("请求飞智游戏列表")?
        .body_mut()
        .read_to_string()
        .context("读取飞智游戏列表")?;
    let value: serde_json::Value = serde_json::from_str(&body).context("解析游戏列表")?;
    let games: Vec<Game> = serde_json::from_value(value["data"].clone()).context("解析游戏列表 data")?;
    paths::write_atomic(&paths::game_list_file(), serde_json::to_string_pretty(&value["data"])?.as_bytes())?;
    Ok(games)
}

/// 读缓存的列表；从未拉取过返回空列表。
pub fn load_cached() -> Result<Vec<Game>> {
    let path = paths::game_list_file();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&text).with_context(|| format!("解析 {}", path.display()))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notice_splits_images() {
        let parts = parse_notice("提示\n<img src='https://a/b.png' />\n<b>后面</b>");
        assert_eq!(
            parts,
            vec![
                NoticePart::Text("提示".into()),
                NoticePart::Image("https://a/b.png".into()),
                NoticePart::Text("后面".into()),
            ]
        );
    }
}
