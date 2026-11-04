//! 手柄线程：所有写 hidraw 的操作都经过这里，负责断线重连和同侧效果去重。

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use flydigi_core::ipc::ControllerView;
use flydigi_proto::{Controller, ForceTrigger, GripBind, TriggerSide};

pub enum DeviceCommand {
    Effect(ForceTrigger),
    Bind(GripBind),
    /// 握把马达（左低频、右高频）。
    Rumble(u8, u8),
    Reset,
}

#[derive(Clone)]
pub struct DeviceHandle {
    tx: Sender<DeviceCommand>,
    pub controllers: Arc<Mutex<Vec<ControllerView>>>,
}

impl DeviceHandle {
    pub fn send(&self, cmd: DeviceCommand) {
        let _ = self.tx.send(cmd);
    }

    pub fn effects(&self, effects: Vec<ForceTrigger>) {
        for e in effects {
            self.send(DeviceCommand::Effect(e));
        }
    }
}

pub fn spawn() -> DeviceHandle {
    let (tx, rx) = channel();
    let controllers = Arc::new(Mutex::new(Vec::new()));
    let handle = DeviceHandle { tx, controllers: controllers.clone() };
    std::thread::Builder::new()
        .name("device".into())
        .spawn(move || run(rx, controllers))
        .expect("启动手柄线程");
    handle
}

struct Pad {
    ctl: Controller,
    /// 每侧最后一次发出的效果；手柄会保持状态，重复的就不再发。
    last: [Option<ForceTrigger>; 2],
}

fn rescan(pads: &mut Vec<Pad>, views: &Mutex<Vec<ControllerView>>) {
    let found = flydigi_proto::discover();
    pads.retain(|p| found.iter().any(|f| f.node == p.ctl.info.node));
    for info in found {
        if pads.iter().any(|p| p.ctl.info.node == info.node) {
            continue;
        }
        match Controller::open(info.clone()) {
            Ok(ctl) => {
                log::info!("手柄已连接：{} ({})", info.name, info.node.display());
                pads.push(Pad { ctl, last: [None, None] });
            }
            Err(e) => log::warn!("{e:#}"),
        }
    }
    let mut v = views.lock().unwrap();
    let before = v.len();
    *v = pads
        .iter_mut()
        .map(|p| ControllerView {
            node: p.ctl.info.node.display().to_string(),
            name: p.ctl.info.name.clone(),
            product_id: p.ctl.info.product_id,
            device_code: v
                .iter()
                .find(|old| old.node == p.ctl.info.node.display().to_string())
                .and_then(|old| old.device_code)
                .or_else(|| p.ctl.device_code().ok()),
        })
        .collect();
    if before > 0 && v.is_empty() {
        log::info!("手柄已断开");
    }
}

fn run(rx: Receiver<DeviceCommand>, views: Arc<Mutex<Vec<ControllerView>>>) {
    let mut pads: Vec<Pad> = Vec::new();
    let mut last_scan = Instant::now() - Duration::from_secs(10);
    loop {
        if last_scan.elapsed() >= Duration::from_secs(2) {
            rescan(&mut pads, &views);
            last_scan = Instant::now();
        }
        let cmd = match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(cmd) => cmd,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let mut failed = false;
        for pad in &mut pads {
            let result = match &cmd {
                DeviceCommand::Effect(effect) => {
                    let slot = &mut pad.last[effect.side as usize - 1];
                    if slot.as_ref() == Some(effect) {
                        continue;
                    }
                    *slot = Some(effect.clone());
                    pad.ctl.set_trigger(effect)
                }
                DeviceCommand::Bind(bind) => pad.ctl.bind_grip(bind),
                DeviceCommand::Rumble(left, right) => pad.ctl.rumble(*left, *right),
                DeviceCommand::Reset => {
                    pad.last = [
                        Some(ForceTrigger::normal(TriggerSide::Left)),
                        Some(ForceTrigger::normal(TriggerSide::Right)),
                    ];
                    pad.ctl.reset_triggers()
                }
            };
            if let Err(e) = result {
                log::warn!("写手柄失败：{e:#}");
                failed = true;
            }
            // 官方左右两侧的命令之间也有间隔，连续写太快接收器可能丢包
            std::thread::sleep(Duration::from_millis(8));
        }
        if failed {
            pads.clear();
            last_scan = Instant::now() - Duration::from_secs(10);
        }
    }
}
