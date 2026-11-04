//! 扳机适配：官方适配游戏的卡片墙和详情面板。

use eframe::egui::{self, Color32, CornerRadius, Margin, RichText, Sense, Stroke, StrokeKind, UiBuilder, vec2};
use flydigi_core::gamelist::{NoticePart, Route};
use flydigi_core::ipc::{GameView, Request, Status};

use crate::i18n::tr;
use crate::app::{App, Filter};
use crate::widgets::{self, ACCENT, BAD, CARD, CARD_HOVER, GOOD, INFO, TEXT_DIM, WARN, route_color, route_icon};

/// 卡片最小宽度；实际宽度按可用空间均分，每行正好铺满。
const CARD_MIN_W: f32 = 220.0;
const CARD_GAP: f32 = 14.0;
/// 封面下方文字区的高度
const CARD_TEXT_H: f32 = 80.0;

fn route_help(route: Route) -> &'static str {
    match route {
        Route::Vibration => tr("游戏运行时让扳机跟随握把震动（命令 82），不需要 mod。"),
        Route::Monitor => tr("官方 XGameMonitor 按 JSON 配置读取游戏内存，根据角色动作切换扳机效果。用 dotnet 运行原版程序，XHelper.dll 换成 Linux 版。"),
        Route::Telemetry => tr("官方 ForzaDualSense 接收游戏的 Data Out 遥测并转换成扳机效果，用 dotnet 运行原版程序。"),
        Route::ExternalExe => tr("官方外置 mod 程序：原生 Windows 程序用 Wine 运行，.NET 程序用 dotnet，经 UDP 7878 发送扳机指令。"),
        Route::InGameMod => tr("官方 mod 文件放进游戏目录，在游戏进程内经 UDP 7878 发送扳机指令。被覆盖的原文件会备份，卸载时还原。"),
        Route::DsOnly => tr("官方只提供 DS 模式。开启全局 DS 模式后，游戏会把手柄当成 DualSense，并直接发送自适应扳机效果。"),
    }
}

fn route_tip(route: Route) -> Option<&'static str> {
    match route {
        Route::Telemetry => Some(tr("游戏内设置：HUD 与游戏玩法 → 数据输出：开，IP 127.0.0.1，端口 5300。")),
        Route::Monitor => Some(tr("带反作弊的游戏请按官方说明离线运行（关闭 EAC 等）。")),
        _ => None,
    }
}

pub fn show(app: &mut App, ui: &mut egui::Ui, games: &[GameView], status: Option<&Status>) {
    let refreshing = status.is_some_and(|s| s.jobs.iter().any(|(id, _)| *id == 0));
    ui.horizontal(|ui| {
        widgets::page_title(ui, tr("🎯  扳机适配"), &format!("{} {}", tr("飞智官方适配的游戏："), games.len()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add_enabled(!refreshing, egui::Button::new(tr("⟳ 更新游戏列表"))).clicked() {
                app.send(Request::RefreshList);
            }
            if refreshing {
                ui.spinner();
            }
        });
    });
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.search)
                .hint_text(tr("🔍 搜索游戏"))
                .desired_width(240.0)
                .margin(Margin::symmetric(8, 5)),
        );
        ui.add_space(8.0);
        for (f, label) in [
            (Filter::All, tr("全部")),
            (Filter::Enabled, tr("已启用")),
            (Filter::Installed, tr("已安装")),
            (Filter::Supported, tr("可用")),
        ] {
            ui.selectable_value(&mut app.filter, f, label);
        }
    });
    ui.add_space(6.0);

    let needle = app.search.to_lowercase();
    let mut visible: Vec<&GameView> = games
        .iter()
        .filter(|g| {
            needle.is_empty()
                || g.game.game_name.to_lowercase().contains(&needle)
                || g.game.en_game_name.to_lowercase().contains(&needle)
        })
        .filter(|g| match app.filter {
            Filter::All => true,
            Filter::Enabled => g.state.enabled,
            Filter::Installed => g.state.installed.is_some(),
            Filter::Supported => g.route.supported(),
        })
        .collect();
    // 运行中、已启用的排前面
    visible.sort_by_key(|g| (!g.running, !g.state.enabled, g.route == Route::DsOnly));

    let busy = |id: i64| status.is_some_and(|s| s.jobs.iter().any(|(j, _)| *j == id));
    let ds_on = status.is_some_and(|s| s.ds_mode.enabled);

    // 详情以叠加层浮在卡片墙右侧，不占卡片墙的宽度，开关时卡片位置不变
    let overlay_area = egui::Rect::from_min_max(egui::pos2(ui.max_rect().left(), ui.cursor().top()), ui.max_rect().max);

    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        let available = ui.available_width() - 2.0;
        let columns = ((available + CARD_GAP) / (CARD_MIN_W + CARD_GAP)).floor().max(1.0) as usize;
        let card_w = ((available - CARD_GAP * (columns - 1) as f32) / columns as f32).floor();
        ui.spacing_mut().item_spacing = vec2(CARD_GAP, CARD_GAP);
        for row in visible.chunks(columns) {
            ui.horizontal(|ui| {
                for view in row {
                    game_card(app, ui, view, card_w, busy(view.game.id), ds_on);
                }
            });
        }
    });

    detail_overlay(app, ui, overlay_area, games, status, ds_on);
}

const DETAIL_W: f32 = 380.0;

fn detail_overlay(app: &mut App, ui: &mut egui::Ui, area: egui::Rect, games: &[GameView], status: Option<&Status>, ds_on: bool) {
    // 大图打开时 Esc 只关大图
    if app.lightbox.is_none() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.selected = None;
    }
    if app.selected.is_some() {
        app.detail_last = app.selected;
    }
    // 关闭时用上一次的游戏把滑出动画播完
    let open = ui.ctx().animate_bool_with_time(egui::Id::unique("game_detail_slide"), app.selected.is_some(), 0.16);
    if open <= 0.0 {
        return;
    }
    let Some(view) = app.detail_last.and_then(|id| games.iter().find(|g| g.game.id == id)).cloned() else {
        return;
    };
    let busy = status.is_some_and(|s| s.jobs.iter().any(|(j, _)| *j == view.game.id));
    let width = DETAIL_W.min(area.width());
    let eased = 1.0 - (1.0 - open).powi(3);
    let pos = egui::pos2(area.right() - width * eased, area.top());
    egui::Area::new(egui::Id::unique("game_detail"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .constrain(false)
        .show(ui.ctx(), |ui| {
            egui::Frame::window(ui.style())
                .fill(widgets::BG)
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::same(14))
                .show(ui, |ui| {
                    ui.set_width(width - 28.0);
                    ui.set_height(area.height() - 30.0);
                    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| detail(app, ui, &view, status, busy, ds_on));
                });
        });
}

fn install_state(view: &GameView) -> Option<(String, Color32)> {
    if !view.route.needs_download() {
        return None;
    }
    Some(match (&view.state.installed, view.update_available) {
        (Some(_), true) => (tr("有更新").into(), WARN),
        (Some(_), false) => (tr("已安装").into(), GOOD),
        (None, _) => (tr("未安装").into(), TEXT_DIM),
    })
}

fn banner(ui: &mut egui::Ui, view: &GameView, width: f32) {
    let size = vec2(width, width * 215.0 / 460.0);
    let url = view.game.image_path.trim();
    if url.starts_with("http") {
        widgets::cover_image(ui, url, size, CornerRadius { nw: 10, ne: 10, sw: 0, se: 0 }, route_color(view.route));
    } else {
        widgets::placeholder_banner(ui, size, view.game.display_name(), route_color(view.route));
    }
}

fn game_card(app: &mut App, ui: &mut egui::Ui, view: &GameView, card_w: f32, busy: bool, ds_on: bool) {
    // Steam 横幅比例 460×215
    let card_h = card_w * 215.0 / 460.0 + CARD_TEXT_H;
    let (rect, response) = ui.allocate_exact_size(vec2(card_w, card_h), Sense::click());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let selected = app.selected == Some(view.game.id);
    let fill = if response.hovered() || selected { CARD_HOVER } else { CARD };
    ui.painter().rect_filled(rect, CornerRadius::same(10), fill);
    let border = if selected {
        Stroke::new(2.0, ACCENT)
    } else if view.running {
        Stroke::new(2.0, INFO)
    } else {
        Stroke::new(1.0, Color32::from_rgb(0x2C, 0x31, 0x3B))
    };
    ui.painter().rect_stroke(rect, CornerRadius::same(10), border, StrokeKind::Inside);
    if response.clicked() {
        app.selected = if selected { None } else { Some(view.game.id) };
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let mut child = ui.new_child(UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)));
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    let ui = &mut child;
    ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
    banner(ui, view, card_w);
    if view.running {
        let badge = egui::Rect::from_min_size(rect.min + vec2(8.0, 8.0), vec2(64.0, 20.0));
        ui.painter().rect_filled(badge, CornerRadius::same(10), INFO);
        ui.painter().text(badge.center(), egui::Align2::CENTER_CENTER, tr("▶ 运行中"), egui::FontId::proportional(12.0), Color32::BLACK);
    }
    egui::Frame::new().inner_margin(Margin::symmetric(10, 6)).show(ui, |ui| {
        ui.set_width(card_w - 20.0);
        ui.add(egui::Label::new(RichText::new(view.game.display_name()).strong().color(Color32::WHITE)).truncate());
        ui.add(egui::Label::new(RichText::new(&view.game.en_game_name).size(11.0).color(TEXT_DIM)).truncate());
        ui.horizontal(|ui| {
            let color = route_color(view.route);
            widgets::pill(ui, format!("{} {}", route_icon(view.route), tr(view.route.label())), color);
            if let Some((text, color)) = install_state(view) {
                widgets::pill(ui, text, color);
            } else if view.route == Route::DsOnly && ds_on {
                widgets::pill(ui, tr("DS 已开启"), ACCENT);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if busy {
                    ui.spinner();
                } else if view.route.supported() {
                    let mut enabled = view.state.enabled;
                    if widgets::toggle(ui, &mut enabled).changed() {
                        app.send(Request::SetEnabled { id: view.game.id, enabled });
                    }
                } else if view.error.is_some() {
                    ui.label(RichText::new("⚠").color(BAD));
                }
            });
        });
    });
}

fn detail(app: &mut App, ui: &mut egui::Ui, view: &GameView, status: Option<&Status>, busy: bool, ds_on: bool) {
    let g = &view.game;
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            if ui.button("✕").clicked() {
                app.selected = None;
            }
        });
    });
    banner(ui, view, ui.available_width());
    ui.add_space(4.0);
    ui.label(RichText::new(g.display_name()).size(20.0).strong().color(Color32::WHITE));
    if !g.en_game_name.is_empty() && g.en_game_name != g.game_name {
        ui.label(RichText::new(&g.en_game_name).color(TEXT_DIM));
    }
    ui.horizontal(|ui| {
        widgets::pill(ui, format!("{} {}", route_icon(view.route), tr(view.route.label())), route_color(view.route));
        if let Some((text, color)) = install_state(view) {
            widgets::pill(ui, text, color);
        }
        if g.applies_vibration() && view.route != Route::Vibration {
            widgets::pill(ui, tr("〰 附带震动绑定"), route_color(Route::Vibration));
        }
    });
    ui.add_space(4.0);
    ui.label(route_help(view.route));
    if let Some(tip) = route_tip(view.route) {
        widgets::callout(ui, INFO, tip);
    }
    if view.route == Route::DsOnly && !ds_on {
        widgets::callout(ui, WARN, tr("全局 DS 模式未开启，可以在左侧「DS 模式」页打开。"));
    }

    if let Some(s) = status {
        let warn = match view.route {
            Route::Monitor | Route::Telemetry if !s.dotnet_ok => Some(tr("找不到 dotnet，请在「服务」页设置").to_owned()),
            Route::Monitor if !s.ptrace_ok => Some(s.ptrace_note.clone()),
            Route::ExternalExe if !s.wine_ok => Some(tr("找不到 wine；原生 Windows 程序的 mod 将无法运行").to_owned()),
            _ => None,
        };
        if let Some(w) = warn {
            widgets::callout(ui, WARN, format!("⚠ {w}"));
        }
    }

    if !g.scenes.trim().is_empty() {
        widgets::section(ui, tr("扳机效果"));
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(egui::Label::new(g.scenes.trim()).wrap());
        });
    }
    let notice = g.notice(crate::i18n::is_english());
    if !notice.is_empty() {
        widgets::section(ui, tr("注意事项"));
        let width = ui.available_width();
        for part in notice {
            match part {
                NoticePart::Text(text) => {
                    ui.add(egui::Label::new(text).wrap());
                }
                NoticePart::Image(url) => {
                    let image = egui::Image::new(url.as_str())
                        .max_width(width)
                        .corner_radius(CornerRadius::same(6))
                        .sense(Sense::click());
                    let response = ui.add(image).on_hover_cursor(egui::CursorIcon::ZoomIn);
                    if response.clicked() {
                        app.lightbox = Some(url);
                    }
                }
            }
        }
    }
    if !g.update.trim().is_empty() {
        widgets::section(ui, tr("更新说明"));
        ui.add(egui::Label::new(RichText::new(g.update.trim()).color(TEXT_DIM)).wrap());
    }

    widgets::section(ui, tr("信息"));
    egui::Grid::new("detail_info").num_columns(2).spacing([14.0, 6.0]).show(ui, |ui| {
        let row = |ui: &mut egui::Ui, k: &str, v: String| {
            ui.label(RichText::new(k).color(TEXT_DIM));
            ui.add(egui::Label::new(v).wrap());
            ui.end_row();
        };
        row(ui, tr("进程名"), g.process_names().join(", "));
        if !g.mod_name.is_empty() {
            row(ui, tr("官方 mod"), g.mod_name.clone());
        }
        if view.route.needs_download() {
            row(ui, tr("最新版本"), format!("{}（{}）", g.version, g.update_time_str));
            row(ui, tr("已装版本"), view.state.installed.as_ref().map_or("—".into(), |i| i.version.clone()));
        }
    });

    if view.route == Route::InGameMod {
        widgets::section(ui, tr("游戏目录"));
        ui.label(RichText::new(tr("mod 会解压到这里，默认按 Steam 库自动查找")).color(TEXT_DIM));
        if app.dir_edit_for != Some(g.id) {
            app.dir_edit_for = Some(g.id);
            app.dir_edit = view.game_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        }
        ui.add(egui::TextEdit::singleline(&mut app.dir_edit).hint_text(tr("未找到，请手动填写")).desired_width(f32::INFINITY));
        ui.horizontal(|ui| {
            if ui.button(tr("保存目录")).clicked() {
                let dir = (!app.dir_edit.trim().is_empty()).then(|| app.dir_edit.trim().into());
                app.send(Request::SetGameDir { id: g.id, dir });
            }
            if view.state.game_dir.is_some() && ui.button(tr("改回自动查找")).clicked() {
                app.send(Request::SetGameDir { id: g.id, dir: None });
                app.dir_edit_for = None;
            }
        });
    }

    widgets::section(ui, tr("操作"));
    ui.add_enabled_ui(!busy && view.route.supported(), |ui| {
        ui.horizontal(|ui| {
            let mut enabled = view.state.enabled;
            if widgets::toggle(ui, &mut enabled).changed() {
                app.send(Request::SetEnabled { id: g.id, enabled });
            }
            ui.label(if view.state.enabled { tr("已启用：检测到游戏时自动生效") } else { tr("未启用") });
        });
        if view.route.needs_download() {
            ui.horizontal(|ui| {
                let label = match (&view.state.installed, view.update_available) {
                    (None, _) => tr("⬇ 下载安装"),
                    (Some(_), true) => tr("⬆ 更新"),
                    (Some(_), false) => tr("⟳ 重新安装"),
                };
                let primary = egui::Button::new(RichText::new(label).color(Color32::WHITE)).fill(ACCENT);
                if ui.add(primary).clicked() {
                    app.send(Request::Install { id: g.id });
                }
                if view.state.installed.is_some() && ui.button(tr("🗑 卸载")).clicked() {
                    app.send(Request::Uninstall { id: g.id });
                }
            });
        }
    });
    if busy {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new(tr("处理中…")).color(TEXT_DIM));
        });
    }
    if let Some(e) = &view.error {
        widgets::callout(ui, BAD, e.clone());
    }
    if let Some(active) = status.and_then(|s| s.active.iter().find(|a| a.id == g.id)) {
        widgets::section(ui, tr("运行状态"));
        widgets::callout(ui, INFO, format!("{} {}", tr("游戏 pid"), active.pid));
        if let Some(cmd) = &active.mod_command {
            ui.label(RichText::new(cmd).monospace().size(11.0).color(TEXT_DIM));
        }
    }
}

/// 全窗口查看大图：暗色背景，图片按窗口缩放居中，点击任意处或按 Esc 关闭。
pub fn lightbox(app: &mut App, ctx: &egui::Context) {
    let Some(url) = app.lightbox.clone() else {
        return;
    };
    let screen = ctx.content_rect();
    let mut close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    egui::Area::new(egui::Id::unique("notice_lightbox"))
        .order(egui::Order::Tooltip)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(rect, CornerRadius::ZERO, Color32::from_black_alpha(215));
            let max = rect.size() * 0.92 - vec2(0.0, 30.0);
            match egui::Image::new(url.as_str()).load_for_size(ctx, max) {
                Ok(egui::load::TexturePoll::Ready { texture }) if texture.size.x > 0.0 => {
                    // 缩小到能放下，小图最多放大两倍
                    let scale = (max.x / texture.size.x).min(max.y / texture.size.y).min(2.0);
                    let size = texture.size * scale;
                    let image_rect = egui::Rect::from_center_size(rect.center() - vec2(0.0, 12.0), size);
                    ui.put(image_rect, egui::Image::from_texture(texture).fit_to_exact_size(size).corner_radius(CornerRadius::same(6)));
                }
                Ok(_) => {
                    ui.put(egui::Rect::from_center_size(rect.center(), vec2(32.0, 32.0)), egui::Spinner::new().size(32.0));
                }
                Err(e) => {
                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, e.to_string(), egui::FontId::proportional(14.0), BAD);
                }
            }
            ui.painter().text(
                egui::pos2(rect.center().x, rect.bottom() - 22.0),
                egui::Align2::CENTER_CENTER,
                tr("点击任意处或按 Esc 关闭"),
                egui::FontId::proportional(13.0),
                Color32::from_gray(190),
            );
            close |= response.clicked();
        });
    if close {
        app.lightbox = None;
    }
}
