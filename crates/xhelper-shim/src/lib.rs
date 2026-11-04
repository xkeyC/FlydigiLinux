//! 飞智 XGameMonitor 的 `XHelper.dll` 的 Linux 实现。
//!
//! XGameMonitor 本身是 .NET 6 托管程序，可以直接在 Linux 的 dotnet 上运行；只有它
//! P/Invoke 的 XHelper.dll 用了 Windows 的 OpenProcess / ReadProcessMemory。把本库命名为
//! `XHelper.dll.so` 放在 XGameMonitor.dll 旁边，.NET 在 Linux 上解析 `DllImport("XHelper.dll")`
//! 时就会加载它，官方程序和配置都不用改。
//!
//! 导出函数与原版一致（x64 上 stdcall 即 C 调用约定）：
//! - `BOOL xHelperInit(const char* processName, const char* moduleName)`
//! - `uint32_t readXData(const int32_t* offsets, int32_t length)`
//!
//! readXData 的指针链语义按原版反汇编：
//! ```text
//! v = read_u64(module_base + offsets[0]); 读失败或为 0 -> 返回 0
//! 其后每一级：v = read_u64((v != 0 ? v : module_base) + offset)，读失败当 0
//! 返回 v 的低 32 位
//! ```
//! 读别的进程的 /proc/<pid>/mem 需要 ptrace 权限：`kernel.yama.ptrace_scope=0`，
//! 或者进程带 CAP_SYS_PTRACE（flydigid 会把它作为 ambient capability 传给 mod 进程）。

use std::ffi::{CStr, c_char};
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::sync::Mutex;

struct Target {
    mem: File,
    module_base: u64,
}

static TARGET: Mutex<Option<Target>> = Mutex::new(None);

fn exe_name(name: &str) -> String {
    let name = name.trim().to_lowercase();
    if name.ends_with(".exe") || name.ends_with(".dll") { name } else { format!("{name}.exe") }
}

/// 某个 pid 里 `file` 的最低映射地址，即 Wine 加载 PE 的基址（等同 Module32Next 的 modBaseAddr）。
fn module_base(pid: &str, file: &str) -> Option<u64> {
    let maps = std::fs::read_to_string(format!("/proc/{pid}/maps")).ok()?;
    maps.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let range = parts.next()?;
            let path = parts.nth(4)?;
            let base = path.rsplit('/').next()?;
            base.eq_ignore_ascii_case(file)
                .then(|| u64::from_str_radix(range.split('-').next()?, 16).ok())
                .flatten()
        })
        .min()
}

fn find_target(process: &str, module: &str) -> Option<Target> {
    let process_exe = exe_name(process);
    let module_file = if module.trim().is_empty() { process_exe.clone() } else { exe_name(module) };
    for entry in std::fs::read_dir("/proc").ok()?.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().filter(|s| s.bytes().all(|b| b.is_ascii_digit())) else {
            continue;
        };
        if module_base(pid, &process_exe).is_none() {
            continue;
        }
        let Some(base) = module_base(pid, &module_file) else {
            continue;
        };
        match File::open(format!("/proc/{pid}/mem")) {
            Ok(mem) => {
                eprintln!("XHelper: Found module: {module_file} pid={pid} base=0x{base:x}");
                return Some(Target { mem, module_base: base });
            }
            Err(e) => eprintln!("XHelper: 打开 /proc/{pid}/mem 失败：{e}（需要 ptrace_scope=0 或 CAP_SYS_PTRACE）"),
        }
    }
    None
}

fn read_u64(mem: &File, address: u64) -> Option<u64> {
    if address == 0 || address > 0x7FFF_FFFF_FFFF {
        return None;
    }
    let mut buf = [0u8; 8];
    mem.read_exact_at(&mut buf, address).ok()?;
    Some(u64::from_le_bytes(buf))
}

fn read_chain(target: &Target, offsets: &[i32]) -> u32 {
    let Some((first, rest)) = offsets.split_first() else {
        return 0;
    };
    let base = target.module_base;
    let mut value = match read_u64(&target.mem, base.wrapping_add_signed(*first as i64)) {
        Some(v) if v != 0 => v,
        _ => return 0,
    };
    for offset in rest {
        let from = if value != 0 { value } else { base };
        value = read_u64(&target.mem, from.wrapping_add_signed(*offset as i64)).unwrap_or(0);
    }
    value as u32
}

/// # Safety
/// 两个参数必须是有效的 NUL 结尾字符串或空指针，由 .NET 的 Ansi 封送保证。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xHelperInit(process_name: *const c_char, module_name: *const c_char) -> i32 {
    let text = |p: *const c_char| {
        if p.is_null() { String::new() } else { unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned() }
    };
    let (process, module) = (text(process_name), text(module_name));
    let target = find_target(&process, &module);
    let ok = target.is_some();
    *TARGET.lock().unwrap_or_else(|e| e.into_inner()) = target;
    ok as i32
}

/// # Safety
/// `list` 必须指向至少 `length` 个 int32。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn readXData(list: *const i32, length: i32) -> u32 {
    if list.is_null() || length <= 0 {
        return 0;
    }
    let offsets = unsafe { std::slice::from_raw_parts(list, length as usize) };
    let guard = TARGET.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_ref().map_or(0, |t| read_chain(t, offsets))
}
