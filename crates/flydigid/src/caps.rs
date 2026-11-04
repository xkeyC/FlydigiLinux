//! XGameMonitor 读游戏内存需要 ptrace 权限。
//!
//! `kernel.yama.ptrace_scope=1`（多数发行版默认）时，同用户也只能读自己子进程的内存。
//! 如果 flydigid 通过文件能力拿到了 CAP_SYS_PTRACE（NixOS 上用 security.wrappers 设
//! `cap_sys_ptrace+eip`），这里把它提升为 ambient capability，dotnet 等 mod 子进程在 exec 后仍然保有它。

const CAP_SYS_PTRACE: u32 = 19;
const LINUX_CAPABILITY_VERSION_3: u32 = 0x2008_0522;

#[repr(C)]
struct CapHeader {
    version: u32,
    pid: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CapData {
    effective: u32,
    permitted: u32,
    inheritable: u32,
}

fn ptrace_scope() -> Option<u32> {
    std::fs::read_to_string("/proc/sys/kernel/yama/ptrace_scope").ok()?.trim().parse().ok()
}

/// 返回 (是否可读其他进程内存, 说明)。
pub fn setup_ptrace() -> (bool, String) {
    let scope = ptrace_scope();
    if matches!(scope, None | Some(0)) {
        return (true, "ptrace_scope=0，可以读取游戏内存".into());
    }
    let mut header = CapHeader { version: LINUX_CAPABILITY_VERSION_3, pid: 0 };
    let mut data = [CapData::default(); 2];
    // SAFETY: capget/capset 只读写这两个结构体，布局与内核 v3 ABI 一致。
    unsafe {
        if libc::syscall(libc::SYS_capget, &mut header, data.as_mut_ptr()) != 0 {
            return (false, "capget 失败".into());
        }
        let bit = 1u32 << CAP_SYS_PTRACE;
        if data[0].permitted & bit == 0 {
            return (
                false,
                format!(
                    "ptrace_scope={}，且 flydigid 没有 CAP_SYS_PTRACE：内存监控类游戏（XGameMonitor）无法工作",
                    scope.unwrap_or(1)
                ),
            );
        }
        data[0].inheritable |= bit;
        data[0].effective |= bit;
        if libc::syscall(libc::SYS_capset, &mut header, data.as_ptr()) != 0 {
            return (false, "capset 失败，无法把 CAP_SYS_PTRACE 传给 mod 进程".into());
        }
        if libc::prctl(libc::PR_CAP_AMBIENT, libc::PR_CAP_AMBIENT_RAISE, CAP_SYS_PTRACE as libc::c_ulong, 0, 0) != 0 {
            return (false, "提升 ambient CAP_SYS_PTRACE 失败".into());
        }
    }
    (true, "已通过 CAP_SYS_PTRACE 获得读取游戏内存的权限".into())
}
