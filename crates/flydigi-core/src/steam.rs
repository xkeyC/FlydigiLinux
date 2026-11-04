//! 按 Steam appid 找游戏安装目录，游戏内 mod 要解压到这里。

use std::path::PathBuf;

fn steam_roots() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    [".local/share/Steam", ".steam/steam", ".var/app/com.valvesoftware.Steam/.local/share/Steam"]
        .iter()
        .map(|p| home.join(p))
        .filter(|p| p.join("steamapps").is_dir())
        .collect()
}

/// 所有 Steam 库目录（含 libraryfolders.vdf 里登记的其他盘）。
pub fn libraries() -> Vec<PathBuf> {
    let mut libs = Vec::new();
    for root in steam_roots() {
        if let Ok(vdf) = std::fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            for line in vdf.lines() {
                let parts: Vec<&str> = line.split('"').collect();
                if parts.len() >= 4 && parts[1] == "path" {
                    libs.push(PathBuf::from(parts[3].replace("\\\\", "\\")));
                }
            }
        }
        libs.push(root);
    }
    let mut seen = std::collections::HashSet::new();
    libs.retain(|p| p.join("steamapps").is_dir() && seen.insert(p.canonicalize().unwrap_or(p.clone())));
    libs
}

pub fn game_dir(app_ids: &[u32]) -> Option<PathBuf> {
    for lib in libraries() {
        for id in app_ids {
            let Ok(acf) = std::fs::read_to_string(lib.join(format!("steamapps/appmanifest_{id}.acf"))) else {
                continue;
            };
            let installdir = acf.lines().find_map(|l| {
                let parts: Vec<&str> = l.split('"').collect();
                (parts.len() >= 4 && parts[1] == "installdir").then(|| parts[3].to_owned())
            });
            if let Some(dir) = installdir {
                let path = lib.join("steamapps/common").join(dir);
                if path.is_dir() {
                    return Some(path);
                }
            }
        }
    }
    None
}
