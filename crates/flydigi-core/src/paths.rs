use std::path::PathBuf;

const APP: &str = "flydigi-linux";

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn xdg(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(fallback))
}

/// 游戏列表缓存、下载的 mod、Wine 前缀：`~/.local/share/flydigi-linux`
pub fn data_dir() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share").join(APP)
}

/// 用户设置与 mod 安装记录：`~/.config/flydigi-linux`
pub fn config_dir() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join(APP)
}

pub fn game_list_file() -> PathBuf {
    data_dir().join("gamelist.json")
}

pub fn state_file() -> PathBuf {
    config_dir().join("state.json")
}

/// 每个适配游戏的 mod 解压目录（外置 mod、XGameMonitor 和 Forza 的配置都在这里）。
pub fn mod_dir(game_id: i64) -> PathBuf {
    data_dir().join("mods").join(game_id.to_string())
}

pub fn wine_prefix() -> PathBuf {
    data_dir().join("wineprefix")
}

pub fn socket_path() -> PathBuf {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("/tmp/{APP}-{}", unsafe { libc::getuid() })));
    runtime.join(format!("{APP}.sock"))
}

/// 写文件时先写临时文件再改名，避免 GUI 和服务同时读写时读到半截。
pub fn write_atomic(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(tmp, path)
}
