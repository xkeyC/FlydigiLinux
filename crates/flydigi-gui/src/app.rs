//! 窗口框架：左侧导航、顶部状态栏、按页面分发。

use eframe::egui::{self, Color32, CornerRadius, Margin, RichText};
use flydigi_core::ipc::{Request, Status};
use flydigi_core::store::Settings;
use flydigi_proto::TriggerMode;

use crate::i18n::tr;
use crate::backend::Backend;
use crate::pages;
use crate::widgets::{self, ACCENT, BAD, GOOD, INFO, TEXT_DIM, WARN};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Games,
    DsMode,
    Test,
    Mapping,
    Sticks,
    Vibration,
    Lighting,
    Macro,
    Screen,
    Service,
    Log,
}

struct NavGroup {
    title: &'static str,
    tabs: &'static [Tab],
}

const NAV: [NavGroup; 3] = [
    NavGroup { title: "扳机", tabs: &[Tab::Games, Tab::DsMode, Tab::Test] },
    NavGroup {
        title: "手柄",
        tabs: &[Tab::Mapping, Tab::Sticks, Tab::Vibration, Tab::Lighting, Tab::Macro, Tab::Screen],
    },
    NavGroup { title: "系统", tabs: &[Tab::Service, Tab::Log] },
];

impl Tab {
    fn label(self) -> (&'static str, &'static str) {
        match self {
            Tab::Games => ("🎯", tr("扳机适配")),
            Tab::DsMode => ("🎮", tr("DS 模式")),
            Tab::Test => ("🔧", tr("扳机测试")),
            Tab::Mapping => ("⌨", tr("按键映射")),
            Tab::Sticks => ("🕹", tr("摇杆与体感")),
            Tab::Vibration => ("〰", tr("震动")),
            Tab::Lighting => ("💡", tr("灯效")),
            Tab::Macro => ("⚡", tr("宏")),
            Tab::Screen => ("🖼", tr("屏幕")),
            Tab::Service => ("⚙", tr("服务")),
            Tab::Log => ("📄", tr("日志")),
        }
    }

    /// 还没复刻的官方功能。
    fn todo(self) -> Option<&'static str> {
        match self {
            Tab::Mapping => Some(tr("按键映射、背键与组合键、板载配置 1–4 切换")),
            Tab::Sticks => Some(tr("摇杆死区与曲线、体感（陀螺仪）映射")),
            Tab::Vibration => Some(tr("握把震动强度与扳机震动设置")),
            Tab::Lighting => Some(tr("手柄与充电座灯效")),
            Tab::Macro => Some(tr("宏录制与绑定")),
            Tab::Screen => Some(tr("APEX 5 屏幕图片与动图上传")),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    Enabled,
    Installed,
    Supported,
}

/// 各页面共享的界面状态。
pub struct App {
    pub backend: Backend,
    pub tab: Tab,
    pub search: String,
    pub filter: Filter,
    pub selected: Option<i64>,
    pub dir_edit: String,
    pub dir_edit_for: Option<i64>,
    pub test_side: u8,
    pub test_mode: u8,
    pub test_params: [u8; 5],
    pub settings_edit: Option<Settings>,
    pub error: Option<String>,
    pub lang: crate::i18n::LangPref,
}

impl App {
    pub fn new(ctx: &egui::Context, lang: crate::i18n::LangPref) -> Self {
        widgets::apply_theme(ctx);
        egui_extras::install_image_loaders(ctx);
        // 网络图片走磁盘缓存，只下载一次
        ctx.add_bytes_loader(std::sync::Arc::new(crate::imgcache::DiskCacheLoader::new()));
        Self {
            backend: Backend::spawn(ctx.clone()),
            tab: Tab::Games,
            search: String::new(),
            filter: Filter::All,
            selected: None,
            dir_edit: String::new(),
            dir_edit_for: None,
            test_side: 2,
            test_mode: TriggerMode::Race as u8,
            test_params: [40, 120, 0, 0, 0],
            settings_edit: None,
            error: None,
            lang,
        }
    }

    pub fn send(&self, request: Request) {
        self.backend.send(request);
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let (status, games, offline) = {
            let mut snap = self.backend.snapshot.lock().unwrap();
            if let Some(e) = snap.error.take() {
                self.error = Some(e);
            }
            (snap.status.clone(), snap.games.clone(), snap.offline.clone())
        };

        egui::Panel::left("nav")
            .resizable(false)
            .exact_size(184.0)
            .frame(egui::Frame::new().fill(widgets::BG).inner_margin(Margin::symmetric(12, 14)))
            .show(ui, |ui| self.nav(ui));

        egui::Panel::top("header")
            .frame(egui::Frame::new().fill(widgets::PANEL).inner_margin(Margin::symmetric(16, 10)))
            .show(ui, |ui| self.header(ui, status.as_ref(), offline.as_deref()));

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(widgets::PANEL).inner_margin(Margin::symmetric(18, 8)))
            .show(ui, |ui| {
                if let Some(todo) = self.tab.todo() {
                    let (icon, label) = self.tab.label();
                    widgets::page_title(ui, &format!("{icon}  {label}"), "");
                    widgets::card(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new(tr("尚未复刻")).size(16.0).color(WARN));
                        ui.label(format!("{} {todo}", tr("官方空间站在这里提供：")));
                        ui.label(RichText::new(tr("当前版本优先实现力回馈扳机，其他功能会陆续加入。")).color(TEXT_DIM));
                    });
                    return;
                }
                match self.tab {
                    Tab::Games => pages::games::show(self, ui, &games, status.as_ref()),
                    Tab::DsMode => pages::ds::show(self, ui, status.as_ref()),
                    Tab::Test => pages::device::show(self, ui, status.as_ref()),
                    Tab::Service => pages::service::show(self, ui, status.as_ref()),
                    Tab::Log => pages::service::log(ui, status.as_ref()),
                    _ => {}
                }
            });
    }
}

impl App {
    fn nav(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("◆").size(22.0).color(ACCENT));
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(RichText::new(tr("飞智空间站")).size(17.0).strong().color(Color32::WHITE));
                ui.label(RichText::new("for Linux").size(11.0).color(TEXT_DIM));
            });
        });
        ui.add_space(14.0);
        for group in &NAV {
            ui.label(RichText::new(tr(group.title)).size(11.0).color(TEXT_DIM));
            ui.spacing_mut().item_spacing.y = 2.0;
            for &tab in group.tabs {
                let (icon, label) = tab.label();
                let selected = self.tab == tab;
                let text_color = if selected {
                    Color32::WHITE
                } else if tab.todo().is_some() {
                    Color32::from_gray(110)
                } else {
                    Color32::from_gray(200)
                };
                let resp = egui::Frame::new()
                    .fill(if selected { ACCENT.linear_multiply(0.22) } else { Color32::TRANSPARENT })
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::symmetric(10, 4))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(icon).size(15.0).color(if selected { ACCENT } else { text_color }));
                            ui.label(RichText::new(label).color(text_color));
                        });
                    })
                    .response
                    .interact(egui::Sense::click());
                if resp.clicked() {
                    self.tab = tab;
                }
                if resp.hovered() && !selected {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            }
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.add_space(10.0);
        }
    }

    fn header(&mut self, ui: &mut egui::Ui, status: Option<&Status>, offline: Option<&str>) {
        ui.horizontal(|ui| {
            match (status, offline) {
                (Some(s), _) => {
                    match s.controllers.first() {
                        Some(c) => {
                            let model = if c.device_code == Some(128) { "APEX 5" } else { tr("飞智手柄") };
                            ui.label(RichText::new("🎮").size(18.0).color(GOOD));
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                ui.label(RichText::new(model).strong().color(Color32::WHITE));
                                ui.label(RichText::new(c.name.trim()).size(11.0).color(TEXT_DIM));
                            });
                        }
                        None => {
                            ui.label(RichText::new("🎮").size(18.0).color(TEXT_DIM));
                            ui.label(RichText::new(tr("未连接手柄")).color(WARN));
                        }
                    }
                    ui.add_space(12.0);
                    for a in &s.active {
                        widgets::pill(ui, format!("▶ {}", a.name), INFO);
                    }
                    if s.ds_mode.enabled {
                        widgets::pill(ui, if s.ds_mode.active { tr("DS 模式运行中") } else { tr("DS 模式异常") }, if s.ds_mode.active { ACCENT } else { BAD });
                    }
                    for (_, job) in &s.jobs {
                        ui.spinner();
                        ui.label(RichText::new(job).color(TEXT_DIM));
                    }
                }
                (None, Some(e)) => {
                    ui.label(RichText::new("⚠").color(BAD));
                    ui.label(RichText::new(format!("{} {e}", tr("无法连接 flydigid 服务："))).color(BAD));
                }
                _ => {
                    ui.spinner();
                }
            }
            if let Some(e) = self.error.clone() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("✕").clicked() {
                        self.error = None;
                    }
                    ui.label(RichText::new(format!("{} {e}", tr("操作失败："))).color(BAD));
                });
            }
        });
    }
}
