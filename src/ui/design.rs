use egui::{Color32, Context, CornerRadius, Frame, Margin, RichText, Stroke, Ui};

pub const BG_PRIMARY: Color32 = Color32::from_rgb(10, 18, 24);
pub const BG_SURFACE: Color32 = Color32::from_rgb(19, 29, 38);
pub const BG_ELEVATED: Color32 = Color32::from_rgb(26, 39, 49);
pub const ACCENT_CYAN: Color32 = Color32::from_rgb(67, 210, 225);
pub const ACCENT_ORANGE: Color32 = Color32::from_rgb(255, 171, 74);
pub const ACCENT_GREEN: Color32 = Color32::from_rgb(83, 221, 156);
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(233, 241, 247);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(154, 173, 188);

pub fn apply_theme(ctx: &Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals.panel_fill = BG_PRIMARY;
    style.visuals.window_fill = BG_SURFACE;
    style.visuals.widgets.noninteractive.bg_fill = BG_SURFACE;
    style.visuals.widgets.inactive.bg_fill = BG_ELEVATED;
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(35, 55, 69);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(42, 66, 83);
    style.visuals.selection.bg_fill = Color32::from_rgb(37, 102, 112);
    style.visuals.extreme_bg_color = BG_PRIMARY;
    style.visuals.faint_bg_color = BG_SURFACE;
    style.visuals.override_text_color = Some(TEXT_PRIMARY);
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(10.0, 7.0);
    style.visuals.window_corner_radius = CornerRadius::same(10);
    ctx.set_style(style);
}

pub fn section_frame() -> Frame {
    Frame::new()
        .fill(BG_SURFACE)
        .stroke(Stroke::new(1.0, Color32::from_rgb(43, 64, 78)))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(12))
}

pub fn title(ui: &mut Ui, text: &str, accent: Color32) {
    ui.label(RichText::new(text).strong().size(18.0).color(accent));
}

