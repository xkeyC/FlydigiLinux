//! 官方 AdapterTriggerService 的 UDP 7878：接收官方 mod 发来的扳机指令，转给手柄。

use std::net::UdpSocket;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::daemon::Daemon;

pub fn spawn(daemon: Arc<Daemon>, port: u16) {
    std::thread::Builder::new()
        .name("udp".into())
        .spawn(move || loop {
            match UdpSocket::bind(("127.0.0.1", port)) {
                Ok(sock) => {
                    log::info!("监听 mod 指令 udp://127.0.0.1:{port}");
                    daemon.udp_listening.store(true, Ordering::Relaxed);
                    serve(&daemon, &sock);
                }
                Err(e) => log::error!("监听 UDP {port} 失败：{e}（是不是官方服务或旧版本还在运行？）"),
            }
            daemon.udp_listening.store(false, Ordering::Relaxed);
            std::thread::sleep(Duration::from_secs(5));
        })
        .expect("启动 UDP 线程");
}

fn serve(daemon: &Daemon, sock: &UdpSocket) {
    let mut buf = vec![0u8; 65536];
    let mut last: Vec<u8> = Vec::new();
    loop {
        let n = match sock.recv(&mut buf) {
            Ok(n) => n,
            Err(e) => {
                log::warn!("UDP 接收失败：{e}");
                return;
            }
        };
        let data = &buf[..n];
        // 官方服务丢弃与上一包完全相同的指令
        if data == last.as_slice() {
            continue;
        }
        last = data.to_vec();
        let Some(effects) = flydigi_core::dsx::parse_packet(data) else {
            log::debug!("忽略无法解析的 UDP 包：{}", String::from_utf8_lossy(data));
            continue;
        };
        if effects.is_empty() {
            continue;
        }
        daemon.note_effects(&effects);
        daemon.device.effects(effects);
    }
}
