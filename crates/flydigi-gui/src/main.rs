//! FlydigiLinux 图形界面。所有操作都发给 flydigid 服务执行。

mod app;
mod backend;
mod i18n;
mod imgcache;
mod pages;
mod widgets;

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let lang = i18n::init();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(i18n::tr("飞智空间站 Linux"))
            .with_app_id("flydigi-linux")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([860.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "flydigi-linux",
        options,
        // 中文由 eframe 的 system_fonts 从系统字体里回退
        Box::new(move |cc| Ok(Box::new(app::App::new(&cc.egui_ctx, lang)))),
    )
}
