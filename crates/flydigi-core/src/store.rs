//! 用户状态（启用了哪些游戏、装了哪些 mod）以及官方 mod 包的安装与卸载。
//!
//! 对应 Space Station 的 ModHelper：外置 mod 解压到程序目录（这里是 `mods/<id>/`），
//! 游戏内 mod 解压到游戏目录。官方直接覆盖游戏文件，这里多做一步：被覆盖的原文件先备份，卸载时还原。

use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::gamelist::{Game, Route};
use crate::{paths, steam};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub games: BTreeMap<i64, GameState>,
    pub settings: Settings,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GameState {
    /// 对应官方的 IsOpen：检测到游戏运行时自动启用它的扳机路线。
    pub enabled: bool,
    pub installed: Option<Installed>,
    /// 游戏内 mod 的目标目录；不填则按 Steam appid 自动查找。
    pub game_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Installed {
    pub version: String,
    pub target: PathBuf,
    pub files: Vec<PathBuf>,
    /// 被覆盖的游戏原文件：原路径 -> 备份路径。
    pub backups: Vec<(PathBuf, PathBuf)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// 运行 .NET mod（ForzaDualSense、XGameMonitor 等）的 dotnet。
    pub dotnet: String,
    /// 运行原生 Windows mod 的 wine。
    pub wine: String,
    /// 官方服务监听的 mod 指令端口，所有官方 mod 都写死 7878。
    pub udp_port: u16,
    /// 全局 DS 模式：虚拟一个 DualSense 给所有游戏用，作为没有官方 mod 时的备用方案。
    pub ds_mode: bool,
    /// DS 模式扳机力度 0..=100%。
    pub ds_trigger_strength: u32,
    /// DS 模式握把震动增益 0..=200%。
    pub ds_rumble_gain: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self { dotnet: "dotnet".into(), wine: "wine".into(), udp_port: 7878, ds_mode: false, ds_trigger_strength: 100, ds_rumble_gain: 100 }
    }
}

impl State {
    pub fn load() -> Self {
        std::fs::read_to_string(paths::state_file())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        paths::write_atomic(&paths::state_file(), serde_json::to_string_pretty(self)?.as_bytes())?;
        Ok(())
    }

    pub fn game(&self, id: i64) -> GameState {
        self.games.get(&id).cloned().unwrap_or_default()
    }

    pub fn game_mut(&mut self, id: i64) -> &mut GameState {
        self.games.entry(id).or_default()
    }
}

impl GameState {
    /// 远端版本号比已装的新（官方用的是字符串比较，版本号形如 2026091601）。
    pub fn update_available(&self, game: &Game) -> bool {
        self.installed.as_ref().is_some_and(|i| game.version.as_str() > i.version.as_str())
    }
}

/// 游戏内 mod 要装到哪个目录。
pub fn resolve_game_dir(game: &Game, state: &GameState) -> Option<PathBuf> {
    state.game_dir.clone().or_else(|| steam::game_dir(&game.steam_app_ids()))
}

/// zip 里的路径：统一成 `/` 分隔，拒绝绝对路径和 `..`。
fn safe_relative(name: &str) -> Option<PathBuf> {
    let path = PathBuf::from(name.replace('\\', "/"));
    path.components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
        .then_some(path)
        .filter(|p| p.components().next().is_some())
}

pub fn download(url: &str) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    ureq::get(url)
        .call()
        .with_context(|| format!("下载 {url}"))?
        .into_body()
        .into_reader()
        .read_to_end(&mut body)
        .context("读取 mod 包")?;
    Ok(body)
}

/// 下载并安装官方 mod 包。返回安装记录，调用方负责写进 State。
pub fn install(game: &Game, state: &GameState) -> Result<Installed> {
    let route = game.route();
    if !route.needs_download() {
        bail!("{} 不需要下载 mod", game.display_name());
    }
    let target = if route == Route::InGameMod {
        resolve_game_dir(game, state).context("找不到游戏目录，请先手动指定")?
    } else {
        let dir = paths::mod_dir(game.id);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).with_context(|| format!("清理 {}", dir.display()))?;
        }
        dir
    };
    let archive = download(&game.mod_down_load_url)?;
    extract(game.id, &archive, &target)
        .map(|(files, backups)| Installed { version: game.version.clone(), target, files, backups })
}

fn extract(game_id: i64, archive: &[u8], target: &Path) -> Result<(Vec<PathBuf>, Vec<(PathBuf, PathBuf)>)> {
    let mut zip = zip::ZipArchive::new(Cursor::new(archive)).context("mod 包不是有效的 zip")?;
    let backup_root = paths::data_dir().join("backup").join(game_id.to_string());
    let mut files = Vec::new();
    let mut backups = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let name = match entry.name() {
            Ok(n) => n.into_owned(),
            Err(_) => String::from_utf8_lossy(entry.name_raw()).into_owned(),
        };
        let Some(rel) = safe_relative(&name) else {
            log::warn!("跳过可疑路径 {name}");
            continue;
        };
        let dest = target.join(&rel);
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir)?;
        }
        if dest.exists() {
            let backup = backup_root.join(&rel);
            std::fs::create_dir_all(backup.parent().unwrap_or(&backup_root))?;
            std::fs::copy(&dest, &backup).with_context(|| format!("备份 {}", dest.display()))?;
            backups.push((dest.clone(), backup));
        }
        let mut out = std::fs::File::create(&dest).with_context(|| format!("写入 {}", dest.display()))?;
        std::io::copy(&mut entry, &mut out)?;
        files.push(dest);
    }
    Ok((files, backups))
}

/// 卸载：删掉装进去的文件，还原备份；外置 mod 整个目录删掉。
pub fn uninstall(game: &Game, installed: &Installed) -> Result<()> {
    for file in &installed.files {
        if file.exists() {
            std::fs::remove_file(file).with_context(|| format!("删除 {}", file.display()))?;
        }
    }
    for (original, backup) in &installed.backups {
        std::fs::copy(backup, original).with_context(|| format!("还原 {}", original.display()))?;
    }
    let backup_root = paths::data_dir().join("backup").join(game.id.to_string());
    let _ = std::fs::remove_dir_all(backup_root);
    let mod_dir = paths::mod_dir(game.id);
    if mod_dir.exists() {
        std::fs::remove_dir_all(mod_dir)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zip_paths_are_sanitised() {
        assert_eq!(safe_relative("mods\\f25\\a.exe"), Some(PathBuf::from("mods/f25/a.exe")));
        assert_eq!(safe_relative("../etc/passwd"), None);
        assert_eq!(safe_relative("/abs"), None);
        assert_eq!(safe_relative(""), None);
    }
}
