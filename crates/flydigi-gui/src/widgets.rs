//! 主题和几个自绘小部件：状态药丸、开关、卡片、状态点。

use eframe::egui::{
    self, Color32, CornerRadius, Margin, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};
use flydigi_core::gamelist::Route;

pub const ACCENT: Color32 = Color32::from_rgb(0xFF, 0x7A, 0x1A);
pub const BG: Color32 = Color32::from_rgb(0x12, 0x14, 0x18);
pub const PANEL: Color32 = Color32::from_rgb(0x18, 0x1B, 0x21);
pub const CARD: Color32 = Color32::from_rgb(0x21, 0x25, 0x2D);
pub const CARD_HOVER: Color32 = Color32::from_rgb(0x2A, 0x2F, 0x39);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x8A, 0x92, 0xA0);
pub const GOOD: Color32 = Color32::from_rgb(0x4C, 0xC3, 0x6A);
pub const WARN: Color32 = Color32::from_rgb(0xF2, 0xB1, 0x34);
pub const BAD: Color32 = Color32::from_rgb(0xEF, 0x5B, 0x5B);
pub const INFO: Color32 = Color32::from_rgb(0x4A, 0xA8, 0xFF);

pub fn apply_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = PANEL;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = BG;
    visuals.faint_bg_color = CARD;
    visuals.selection.bg_fill = ACCENT.linear_multiply(0.85);
    visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.hyperlink_color = ACCENT;
    for w in [&mut visuals.widgets.inactive, &mut visuals.widgets.hovered, &mut visuals.widgets.active] {
        w.corner_radius = CornerRadius::same(6);
    }
    visuals.widgets.inactive.weak_bg_fill = CARD;
    visuals.widgets.hovered.weak_bg_fill = CARD_HOVER;
    visuals.widgets.active.weak_bg_fill = ACCENT.linear_multiply(0.6);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x2C, 0x31, 0x3B));
    ctx.set_visuals(visuals);
    ctx.global_style_mut(|style| {
        style.spacing.item_spacing = vec2(8.0, 8.0);
        style.spacing.button_padding = vec2(12.0, 6.0);
        style.spacing.interact_size.y = 26.0;
    });
}

pub fn route_color(route: Route) -> Color32 {
    match route {
        Route::Vibration => Color32::from_rgb(0x4C, 0xC3, 0x6A),
        Route::Monitor => Color32::from_rgb(0xFF, 0x9F, 0x1C),
        Route::Telemetry => Color32::from_rgb(0x4A, 0xA8, 0xFF),
        Route::ExternalExe => Color32::from_rgb(0xB3, 0x7B, 0xFF),
        Route::InGameMod => Color32::from_rgb(0x2E, 0xC4, 0xB6),
        Route::DsOnly => Color32::from_rgb(0x8A, 0x92, 0xA0),
    }
}

pub fn route_icon(route: Route) -> &'static str {
    match route {
        Route::Vibration => "〰",
        Route::Monitor => "👁",
        Route::Telemetry => "📡",
        Route::ExternalExe => "🧩",
        Route::InGameMod => "📦",
        Route::DsOnly => "🎮",
    }
}

/// 圆角小标签。
pub fn pill(ui: &mut Ui, text: impl Into<String>, color: Color32) -> Response {
    egui::Frame::new()
        .fill(color.linear_multiply(0.18))
        .stroke(Stroke::new(1.0, color.linear_multiply(0.55)))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(8, 2))
        .show(ui, |ui| ui.label(RichText::new(text.into()).color(color).size(12.0)))
        .response
}

/// 带文字的状态点。
pub fn status_dot(ui: &mut Ui, ok: bool, text: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 4.5, if ok { GOOD } else { BAD });
        ui.label(text);
    });
}

/// iOS 风格开关。
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let size = vec2(40.0, 22.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool(response.id, *on);
        let off_color = Color32::from_rgb(0x3A, 0x40, 0x4C);
        let bg = lerp_color(off_color, ACCENT, t);
        let radius = rect.height() / 2.0;
        ui.painter().rect_filled(rect, CornerRadius::same(radius as u8), bg);
        let x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), t);
        let knob = if response.hovered() { Color32::WHITE } else { Color32::from_gray(235) };
        ui.painter().circle_filled(pos2(x, rect.center().y), radius - 3.0, knob);
    }
    response
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// 内容卡片。
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(CARD)
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(14))
        .show(ui, add)
        .inner
}

/// 页面标题 + 副标题。
pub fn page_title(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(title).size(22.0).strong().color(Color32::WHITE));
    if !subtitle.is_empty() {
        ui.label(RichText::new(subtitle).color(TEXT_DIM));
    }
    ui.add_space(6.0);
}

pub fn section(ui: &mut Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).size(15.0).strong().color(Color32::WHITE));
}

/// 提示框（警告/说明）。
pub fn callout(ui: &mut Ui, color: Color32, text: impl Into<String>) {
    egui::Frame::new()
        .fill(color.linear_multiply(0.12))
        .stroke(Stroke::new(1.0, color.linear_multiply(0.5)))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(text.into()).color(color));
        });
}

/// 没有封面图时的占位：渐变底色 + 名字首字。
pub fn placeholder_banner(ui: &mut Ui, size: Vec2, name: &str, color: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius { nw: 10, ne: 10, sw: 0, se: 0 }, color.linear_multiply(0.25));
    painter.rect_stroke(rect, CornerRadius { nw: 10, ne: 10, sw: 0, se: 0 }, Stroke::new(1.0, color.linear_multiply(0.4)), StrokeKind::Inside);
    let initial: String = name.chars().take(2).collect();
    painter.text(rect.center(), egui::Align2::CENTER_CENTER, initial, egui::FontId::proportional(size.y * 0.35), color);
    response
}

/// 按「填满后居中裁剪」显示图片，比例不同的图不会被压扁；加载中显示底色。
pub fn cover_image(ui: &mut Ui, url: &str, size: Vec2, radius: CornerRadius, tint: Color32) -> Response {
    match egui::Image::new(url).load_for_size(ui.ctx(), size) {
        Ok(egui::load::TexturePoll::Ready { texture }) if texture.size.x > 0.0 && texture.size.y > 0.0 => {
            let source = texture.size.x / texture.size.y;
            let target = size.x / size.y;
            let uv = if source > target {
                // 图更宽：裁左右
                let w = target / source;
                egui::Rect::from_min_max(pos2((1.0 - w) / 2.0, 0.0), pos2((1.0 + w) / 2.0, 1.0))
            } else {
                // 图更高：裁上下
                let h = source / target;
                egui::Rect::from_min_max(pos2(0.0, (1.0 - h) / 2.0), pos2(1.0, (1.0 + h) / 2.0))
            };
            ui.add(
                egui::Image::from_texture(texture)
                    .uv(uv)
                    .fit_to_exact_size(size)
                    .maintain_aspect_ratio(false)
                    .corner_radius(radius),
            )
        }
        _ => {
            let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
            ui.painter().rect_filled(rect, radius, tint.linear_multiply(0.12));
            response
        }
    }
}
