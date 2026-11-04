//! 全局 DS 模式页。

use eframe::egui::{self, Color32, RichText};
use flydigi_core::ipc::{Request, Status};

use crate::i18n::tr;
use crate::app::App;
use crate::widgets::{self, ACCENT, BAD, GOOD, INFO, TEXT_DIM, WARN};

const LAUNCH_OPTIONS: &str =
    "SDL_GAMECONTROLLER_IGNORE_DEVICES=0x37d7/0x2501 SDL_JOYSTICK_IGNORE_DEVICES=0x37d7/0x2501 %command%";

pub fn show(app: &mut App, ui: &mut egui::Ui, status: Option<&Status>) {
    widgets::page_title(ui, tr("🎮  DS 模式"), tr("虚拟一个 DualSense 手柄给所有游戏，作为没有官方 mod 的游戏的备用方案"));
    let Some(s) = status else {
        widgets::callout(ui, BAD, tr("flydigid 服务未运行"));
        return;
    };
    let ds = &s.ds_mode;
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new("🎮").size(34.0).color(if ds.active { ACCENT } else { TEXT_DIM }));
                ui.vertical(|ui| {
                    ui.label(RichText::new(tr("全局 DS 模式")).size(18.0).strong().color(Color32::WHITE));
                    ui.label(RichText::new(if ds.enabled {
                        if ds.active { tr("运行中：游戏会看到一个 DualSense") } else { tr("已开启，但虚拟手柄没有创建成功") }
                    } else {
                        tr("已关闭")
                    })
                    .color(TEXT_DIM));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut on = ds.enabled;
                    if widgets::toggle(ui, &mut on).changed() {
                        app.send(Request::SetDsMode { enabled: on });
                    }
                });
            });
            if let Some(e) = &ds.error {
                widgets::callout(ui, BAD, e.clone());
            }
            if ds.enabled {
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    widgets::status_dot(ui, ds.active, tr("虚拟 DualSense"));
                    ui.add_space(12.0);
                    widgets::status_dot(ui, ds.pad_connected, tr("实体手柄已独占"));
                    ui.add_space(12.0);
                    widgets::status_dot(ui, ds.motion, tr("体感数据"));
                    ui.add_space(12.0);
                    widgets::status_dot(ui, ds.game_opened, tr("已被程序打开"));
                    ui.add_space(12.0);
                    ui.label(RichText::new(format!("{} {}", tr("收到游戏输出报告"), ds.output_reports)).color(
                        if ds.output_reports > 0 { GOOD } else { TEXT_DIM },
                    ));
                });
            }
        });

        ui.add_space(10.0);
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::section(ui, tr("力度"));
            let edit = app.settings_edit.get_or_insert_with(|| s.settings.clone());
            egui::Grid::new("ds_strength").num_columns(2).spacing([16.0, 10.0]).show(ui, |ui| {
                ui.label(tr("扳机力度"));
                ui.add(egui::Slider::new(&mut edit.ds_trigger_strength, 0..=100).suffix("%"));
                ui.end_row();
                ui.label(tr("握把震动增益"));
                ui.add(egui::Slider::new(&mut edit.ds_rumble_gain, 0..=200).suffix("%"));
                ui.end_row();
            });
            let changed = edit.ds_trigger_strength != s.settings.ds_trigger_strength
                || edit.ds_rumble_gain != s.settings.ds_rumble_gain;
            if ui.add_enabled(changed, egui::Button::new(tr("应用"))).clicked() {
                let settings = edit.clone();
                app.backend.send(Request::SetSettings { settings });
            }
        });

        ui.add_space(10.0);
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::section(ui, tr("游戏设置"));
            ui.label(tr("1. Steam 里对这个游戏关闭 Steam 输入，否则 DualSense 的自适应扳机指令到不了虚拟手柄。"));
            ui.label(tr("2. 在启动选项里填下面这行，让游戏忽略实体手柄、只用虚拟 DualSense："));
            ui.horizontal(|ui| {
                egui::Frame::new()
                    .fill(widgets::BG)
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.add(egui::Label::new(RichText::new(LAUNCH_OPTIONS).monospace().size(12.0)).wrap());
                    });
                if ui.button(tr("📋 复制")).clicked() {
                    ui.ctx().copy_text(LAUNCH_OPTIONS.to_owned());
                }
            });
            ui.add_space(4.0);
            widgets::callout(
                ui,
                INFO,
                tr("参照 ApexSenseBridge：按键和摇杆沿用手柄当前的板载配置，体感来自手柄的原始输入流；自适应扳机按 ASB 的规则翻译成飞智的扳机效果。"),
            );
            widgets::callout(ui, WARN, tr("暂不支持 DualSense 的音频触觉、触摸板滑动和灯条；实体手柄的视图键对应触摸板按下。"));
        });
    });
}
