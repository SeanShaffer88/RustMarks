use egui::{Color32, Stroke};
pub const DEEP: Color32 = Color32::from_rgb(24, 24, 24);
pub const EDITOR: Color32 = Color32::from_rgb(30, 30, 30);
pub const SURFACE: Color32 = Color32::from_rgb(37, 37, 38);
pub const MUTED: Color32 = Color32::from_rgb(133, 133, 133);
pub const ACCENT: Color32 = Color32::from_rgb(0, 122, 204);
pub const GREEN: Color32 = Color32::from_rgb(78, 201, 176);
pub const RED: Color32 = Color32::from_rgb(241, 76, 76);
pub const ORANGE: Color32 = Color32::from_rgb(206, 145, 120);
pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = DEEP;
    style.visuals.window_fill = SURFACE;
    style.visuals.extreme_bg_color = EDITOR;
    style.visuals.override_text_color = Some(Color32::from_rgb(204, 204, 204));
    style.visuals.selection.bg_fill = ACCENT;
    style.visuals.widgets.inactive.bg_fill = SURFACE;
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(42, 45, 46);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(55, 55, 61);
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1., Color32::from_rgb(51, 51, 51));
    style.spacing.item_spacing = egui::vec2(8., 8.);
    ctx.set_style(style);
}
