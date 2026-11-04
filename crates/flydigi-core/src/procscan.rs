//! 在 /proc 里找正在运行的游戏。
//!
//! Proton 下游戏进程的命令行是 Windows 路径（`Z:\…\Game.exe`），而 Steam、pressure-vessel、
//! reaper 等包装进程的命令行里也会带着游戏路径，所以优先认「maps 里真的映射了 exe」的进程。

use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Candidate {
    pub pid: u32,
    /// 进程确实映射了 `<名字>.exe`，是游戏本体而不是包装进程。
    pub mapped: bool,
}

fn basename_lower(token: &str) -> String {
    let token = token.replace('\\', "/");
    let base = token.rsplit('/').next().unwrap_or_default().trim().to_lowercase();
    base.strip_suffix(".exe").map(str::to_owned).unwrap_or(base)
}

fn maps_exe(pid: &str, exe: &str) -> bool {
    let Ok(maps) = std::fs::read_to_string(Path::new("/proc").join(pid).join("maps")) else {
        return false;
    };
    maps.lines()
        .filter_map(|l| l.split_whitespace().nth(5))
        .any(|path| path.rsplit('/').next().is_some_and(|b| b.eq_ignore_ascii_case(exe)))
}

/// 一次扫描：返回 进程名（小写、不带 .exe） -> 匹配到的进程。只关心 `wanted` 里的名字。
pub fn scan(wanted: &[String]) -> HashMap<String, Vec<Candidate>> {
    let mut found: HashMap<String, Vec<Candidate>> = HashMap::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return found;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().filter(|s| s.bytes().all(|b| b.is_ascii_digit())) else {
            continue;
        };
        let Ok(cmdline) = std::fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        for arg in cmdline.split(|b| *b == 0).filter(|a| !a.is_empty()) {
            let base = basename_lower(&String::from_utf8_lossy(arg));
            if !wanted.iter().any(|w| *w == base) {
                continue;
            }
            let mapped = maps_exe(pid, &format!("{base}.exe"));
            let list = found.entry(base).or_default();
            let pid = pid.parse().unwrap_or(0);
            if !list.iter().any(|c| c.pid == pid) {
                list.push(Candidate { pid, mapped });
            }
            break;
        }
    }
    found
}
