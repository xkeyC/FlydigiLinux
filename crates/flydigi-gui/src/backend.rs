//! 与 flydigid 通信的后台线程：定时拉状态和游戏列表，执行界面发来的请求，界面线程从不阻塞。

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use flydigi_core::ipc::{self, GameView, Request, Response, Status};

#[derive(Default)]
pub struct Snapshot {
    pub status: Option<Status>,
    pub games: Vec<GameView>,
    /// 连接不上服务时的错误。
    pub offline: Option<String>,
    /// 最近一次请求失败的错误，界面显示后清除。
    pub error: Option<String>,
}

pub struct Backend {
    pub snapshot: Arc<Mutex<Snapshot>>,
    tx: Sender<Request>,
}

impl Backend {
    pub fn spawn(ctx: eframe::egui::Context) -> Self {
        let snapshot = Arc::new(Mutex::new(Snapshot::default()));
        let (tx, rx) = channel();
        let shared = snapshot.clone();
        std::thread::Builder::new()
            .name("backend".into())
            .spawn(move || run(rx, shared, ctx))
            .expect("启动后台线程");
        Self { snapshot, tx }
    }

    pub fn send(&self, request: Request) {
        let _ = self.tx.send(request);
    }
}

fn run(rx: Receiver<Request>, shared: Arc<Mutex<Snapshot>>, ctx: eframe::egui::Context) {
    let mut last_games = Instant::now() - Duration::from_secs(60);
    loop {
        let mut games_dirty = false;
        if let Ok(request) = rx.recv_timeout(Duration::from_millis(800)) {
            let result = ipc::call(&request);
            if let Err(e) = result {
                shared.lock().unwrap().error = Some(format!("{e:#}"));
            }
            games_dirty = true;
        }
        match ipc::call(&Request::Status) {
            Ok(Response::Status(status)) => {
                // 有后台任务时游戏状态变化快，刷新勤一点
                if !status.jobs.is_empty() {
                    games_dirty = true;
                }
                let mut snap = shared.lock().unwrap();
                let jobs_finished = snap.status.as_ref().is_some_and(|s| s.jobs.len() > status.jobs.len());
                let active_changed = snap.status.as_ref().map(|s| s.active.len()) != Some(status.active.len());
                games_dirty |= jobs_finished || active_changed;
                snap.status = Some(status);
                snap.offline = None;
            }
            Ok(_) => {}
            Err(e) => {
                let mut snap = shared.lock().unwrap();
                snap.status = None;
                snap.offline = Some(format!("{e:#}"));
                drop(snap);
                ctx.request_repaint();
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
        }
        if games_dirty || last_games.elapsed() > Duration::from_secs(5) {
            if let Ok(Response::Games { games }) = ipc::call(&Request::Games) {
                shared.lock().unwrap().games = games;
            }
            last_games = Instant::now();
        }
        ctx.request_repaint();
    }
}
