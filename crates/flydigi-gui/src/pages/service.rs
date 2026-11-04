//! 服务状态、运行环境设置与日志。

use eframe::egui::{self, Color32, RichText};
use flydigi_core::ipc::{Request, Status};

use crate::i18n::{LangPref, tr};
use crate::app::App;
use crate::widgets::{self, BAD, GOOD, INFO, TEXT_DIM, WARN};

pub fn show(app: &mut App, ui: &mut egui::Ui, status: Option<&Status>) {
    widgets::page_title(ui, tr("⚙  服务"), tr("flydigid 在后台复刻 Space Station 服务：接收 mod 指令、检测游戏、启停官方 mod"));
    let Some(s) = status else {
        widgets::callout(ui, BAD, tr("flydigid 未运行。启动方式：systemctl --user start flydigid，或直接运行 flydigid。"));
        return;
    };
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::section(ui, tr("运行状态"));
            egui::Grid::new("svc").num_columns(2).spacing([24.0, 8.0]).show(ui, |ui| {
                widgets::status_dot(ui, s.udp_listening, &format!("{} {}", tr("mod 指令端口 UDP"), s.udp_port));
                ui.label(RichText::new(tr("所有官方 mod 都把扳机指令发到这里")).color(TEXT_DIM));
                ui.end_row();
                widgets::status_dot(ui, s.dotnet_ok, "dotnet");
                ui.label(RichText::new(tr("运行 ForzaDualSense、XGameMonitor")).color(TEXT_DIM));
                ui.end_row();
                widgets::status_dot(ui, s.wine_ok, "wine");
                ui.label(RichText::new(tr("运行 F1 等原生 Windows mod")).color(TEXT_DIM));
                ui.end_row();
                widgets::status_dot(ui, s.ptrace_ok, tr("读取游戏内存"));
                ui.label(RichText::new(tr("XGameMonitor 需要")).color(TEXT_DIM));
                ui.end_row();
            });
            ui.label(RichText::new(&s.ptrace_note).color(TEXT_DIM));
            if !s.ptrace_ok {
                widgets::callout(
                    ui,
                    WARN,
                    tr("内存监控类游戏需要 sysctl kernel.yama.ptrace_scope=0，或给 flydigid 加 cap_sys_ptrace+eip 文件能力（NixOS 模块会自动配置）。"),
                );
            }
            ui.label(RichText::new(format!("{} {}", tr("版本"), s.version)).size(11.0).color(TEXT_DIM));
        });

        ui.add_space(10.0);
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::section(ui, tr("界面"));
            ui.horizontal(|ui| {
                ui.label(tr("界面语言"));
                for (pref, label) in [(LangPref::Auto, tr("跟随系统")), (LangPref::Zh, "中文"), (LangPref::En, "English")] {
                    if ui.selectable_value(&mut app.lang, pref, label).changed() {
                        crate::i18n::set(pref);
                        ui.ctx().request_repaint();
                    }
                }
            });
        });

        ui.add_space(10.0);
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::section(ui, tr("运行环境"));
            let edit = app.settings_edit.get_or_insert_with(|| s.settings.clone());
            egui::Grid::new("settings").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                ui.label(tr("dotnet 命令"));
                ui.add(egui::TextEdit::singleline(&mut edit.dotnet).desired_width(320.0));
                ui.end_row();
                ui.label(tr("wine 命令"));
                ui.add(egui::TextEdit::singleline(&mut edit.wine).desired_width(320.0));
                ui.end_row();
            });
            let (save, revert) = ui
                .horizontal(|ui| {
                    let save = ui.add(egui::Button::new(RichText::new(tr("保存")).color(Color32::WHITE)).fill(widgets::ACCENT)).clicked();
                    (save, ui.button(tr("还原")).clicked())
                })
                .inner;
            if save {
                let settings = edit.clone();
                app.backend.send(Request::SetSettings { settings });
            }
            if revert {
                app.settings_edit = None;
            }
        });
    });
}

pub fn log(ui: &mut egui::Ui, status: Option<&Status>) {
    widgets::page_title(ui, tr("📄  日志"), tr("flydigid 最近的日志"));
    let Some(s) = status else {
        return;
    };
    widgets::card(ui, |ui| {
        ui.set_width(ui.available_width());
        egui::ScrollArea::vertical().auto_shrink(false).stick_to_bottom(true).show(ui, |ui| {
            for line in &s.log {
                let color = if line.contains(" ERROR ") {
                    BAD
                } else if line.contains(" WARN ") {
                    WARN
                } else if line.contains("[") {
                    INFO
                } else if line.contains("检测到游戏") || line.contains("已安装") {
                    GOOD
                } else {
                    Color32::from_gray(200)
                };
                ui.label(RichText::new(line).monospace().size(12.0).color(color));
            }
        });
    });
}
