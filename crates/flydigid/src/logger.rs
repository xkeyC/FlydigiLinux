//! 日志同时写 stderr（systemd journal）和一个环形缓冲区，GUI 通过 Status 查看最近的日志。

use std::collections::VecDeque;
use std::sync::Mutex;

const CAPACITY: usize = 300;

static BUFFER: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

struct Logger {
    level: log::LevelFilter,
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let local = now as i64 + utc_offset();
        let line = format!(
            "{:02}:{:02}:{:02} {:<5} {}",
            (local / 3600) % 24,
            (local / 60) % 60,
            local % 60,
            record.level(),
            record.args()
        );
        eprintln!("{line}");
        let mut buf = BUFFER.lock().unwrap_or_else(|e| e.into_inner());
        if buf.len() == CAPACITY {
            buf.pop_front();
        }
        buf.push_back(line);
    }

    fn flush(&self) {}
}

fn utc_offset() -> i64 {
    // SAFETY: localtime_r 只写传入的 tm。
    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut tm);
        tm.tm_gmtoff
    }
}

pub fn init() {
    let level = std::env::var("FLYDIGI_LOG")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(log::LevelFilter::Info);
    let _ = log::set_boxed_logger(Box::new(Logger { level }));
    log::set_max_level(level);
}

pub fn recent() -> Vec<String> {
    BUFFER.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect()
}
