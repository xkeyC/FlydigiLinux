//! 设备信息与扳机效果测试。

use eframe::egui::{self, Color32, CornerRadius, RichText, Sense, Stroke, StrokeKind, vec2};
use flydigi_core::ipc::{Request, Status};
use flydigi_proto::TriggerMode;

use crate::i18n::tr;
use crate::app::App;
use crate::widgets::{self, ACCENT, GOOD, INFO, TEXT_DIM, WARN};

fn param_names(mode: u8) -> &'static [&'static str] {
    match TriggerMode::from_u8(mode) {
        Some(TriggerMode::Race) => &["起始行程", "阻力", "跟随行程"],
        Some(TriggerMode::Sniper) | Some(TriggerMode::Vibration) => &["起始行程", "压力档", "强度", "频率", "跟随行程"],
        Some(TriggerMode::Recoil) => &["起始行程", "断点行程", "强度", "", "跟随行程"],
        Some(TriggerMode::Lock) => &["锁定位置", "强度", "跟随行程"],
        _ => &[],
    }
}

fn mode_help(mode: u8) -> &'static str {
    match TriggerMode::from_u8(mode) {
        Some(TriggerMode::Normal) => tr("没有阻力，恢复普通扳机。"),
        Some(TriggerMode::Race) => tr("从起始行程开始持续阻力，赛车油门/刹车的手感。"),
        Some(TriggerMode::Sniper) => tr("越过起始行程后持续震颤，像机枪连射。"),
        Some(TriggerMode::Recoil) => tr("先阻挡、到断点后突然松开，像扣下狙击枪扳机。"),
        Some(TriggerMode::Lock) => tr("在锁定位置卡死，扳机轴变成只有按下/松开两档。"),
        Some(TriggerMode::Vibration) => tr("官方软件从未使用过的模式，效果不确定。"),
        None => "",
    }
}

/// 扳机行程示意：0..255 的条，标出起始位置。
fn travel_bar(ui: &mut egui::Ui, start: u8, active: bool) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width().min(420.0), 18.0), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(9), widgets::BG);
    if active {
        let x = rect.left() + rect.width() * start as f32 / 255.0;
        let resist = egui::Rect::from_min_max(egui::pos2(x, rect.top()), rect.max);
        painter.rect_filled(resist, CornerRadius::same(9), ACCENT.linear_multiply(0.6));
        painter.line_segment([egui::pos2(x, rect.top() - 3.0), egui::pos2(x, rect.bottom() + 3.0)], Stroke::new(2.0, Color32::WHITE));
    }
    painter.rect_stroke(rect, CornerRadius::same(9), Stroke::new(1.0, Color32::from_gray(70)), StrokeKind::Inside);
}

pub fn show(app: &mut App, ui: &mut egui::Ui, status: Option<&Status>) {
    widgets::page_title(ui, tr("🔧  扳机测试"), tr("直接下发扳机效果，只改手柄的实时状态，不写入手柄存储"));
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::section(ui, tr("已连接的手柄"));
            match status.map(|s| s.controllers.as_slice()) {
                Some([]) | None => {
                    widgets::callout(ui, WARN, tr("没有检测到飞智手柄的配置接口（usage page 0xFFA0），检查连接和 hidraw 权限。"));
                }
                Some(list) => {
                    for c in list {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("🎮").size(22.0).color(GOOD));
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                let model = if c.device_code == Some(128) { "APEX 5" } else { tr("飞智手柄") };
                                ui.label(RichText::new(model).strong().color(Color32::WHITE));
                                ui.label(
                                    RichText::new(format!(
                                        "{} · {} · PID {:04x} · {} {}",
                                        c.name.trim(),
                                        c.node,
                                        c.product_id,
                                        tr("设备代码"),
                                        c.device_code.map_or("?".into(), |d| d.to_string())
                                    ))
                                    .size(11.0)
                                    .color(TEXT_DIM),
                                );
                            });
                        });
                    }
                }
            }
        });

        ui.add_space(10.0);
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::section(ui, tr("扳机效果"));
            ui.horizontal(|ui| {
                ui.selectable_value(&mut app.test_side, 1, RichText::new(tr("◀ 左扳机 L2")).size(14.0));
                ui.selectable_value(&mut app.test_side, 2, RichText::new(tr("右扳机 R2 ▶")).size(14.0));
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for mode in TriggerMode::ALL {
                    let selected = app.test_mode == mode as u8;
                    let text = RichText::new(tr(mode.label())).color(if selected { Color32::WHITE } else { Color32::from_gray(200) });
                    let button = egui::Button::new(text)
                        .fill(if selected { ACCENT } else { widgets::CARD_HOVER })
                        .min_size(vec2(64.0, 28.0));
                    if ui.add(button).clicked() {
                        app.test_mode = mode as u8;
                    }
                }
            });
            ui.label(RichText::new(mode_help(app.test_mode)).color(TEXT_DIM));
            ui.add_space(4.0);
            let names = param_names(app.test_mode);
            travel_bar(ui, app.test_params[0], app.test_mode != 0);
            egui::Grid::new("params").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                for (i, name) in names.iter().enumerate() {
                    if name.is_empty() {
                        continue;
                    }
                    ui.label(tr(name));
                    if name.contains("跟随") {
                        let mut on = app.test_params[i] != 0;
                        if widgets::toggle(ui, &mut on).changed() {
                            app.test_params[i] = on as u8;
                        }
                    } else {
                        ui.add(egui::Slider::new(&mut app.test_params[i], 0..=255));
                    }
                    ui.end_row();
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let apply = egui::Button::new(RichText::new(tr("应用到手柄")).color(Color32::WHITE)).fill(ACCENT);
                if ui.add(apply).clicked() {
                    app.send(Request::TestTrigger {
                        side: app.test_side,
                        mode: app.test_mode,
                        params: app.test_params[..names.len()].to_vec(),
                    });
                }
                if ui.button(tr("两侧恢复普通")).clicked() {
                    app.send(Request::ResetTriggers);
                }
            });
            widgets::callout(ui, INFO, tr("游戏运行时，mod 或 DS 模式发来的指令会覆盖这里的设置。"));
        });

        if let Some(s) = status {
            ui.add_space(10.0);
            widgets::card(ui, |ui| {
                ui.set_width(ui.available_width());
                widgets::section(ui, tr("最近的扳机指令"));
                if s.last_effects.is_empty() {
                    ui.label(RichText::new(tr("还没有收到指令")).color(TEXT_DIM));
                }
                for e in s.last_effects.iter().rev().take(12) {
                    let mut parts = e.splitn(3, ' ');
                    let (side, mode, params) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
                    let left = side == "L";
                    let color = if left { Color32::from_rgb(0x8C, 0xC8, 0xFF) } else { Color32::from_rgb(0xFF, 0xB8, 0x80) };
                    let mode_label = mode.parse().ok().and_then(TriggerMode::from_u8).map_or("?", |m| tr(m.label()));
                    let side_label = if left { tr("左") } else { tr("右") };
                    ui.label(RichText::new(format!("{side_label}  {mode_label}({mode})  {params}")).monospace().color(color));
                }
            });
        }
    });
}
