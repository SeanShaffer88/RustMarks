//! Reference-inspired charcoal surfaces, with blue reserved for primary actions.
use egui::{Color32, Stroke};
pub const DEEP: Color32 = Color32::from_rgb(25, 25, 25);
pub const EDITOR: Color32 = Color32::from_rgb(31, 31, 31);
pub const SURFACE: Color32 = Color32::from_rgb(39, 39, 39);
pub const HOVER: Color32 = Color32::from_rgb(48, 48, 48);
pub const BORDER: Color32 = Color32::from_rgb(56, 56, 56);
pub const TEXT: Color32 = Color32::from_rgb(228, 228, 228);
pub const MUTED: Color32 = Color32::from_rgb(155, 155, 155);
pub const ACCENT: Color32 = Color32::from_rgb(0, 132, 245);
pub const GREEN: Color32 = Color32::from_rgb(116, 201, 164);
pub const RED: Color32 = Color32::from_rgb(235, 128, 128);
pub const ORANGE: Color32 = Color32::from_rgb(220, 181, 122);
pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = DEEP;
    style.visuals.window_fill = SURFACE;
    style.visuals.extreme_bg_color = EDITOR;
    style.visuals.faint_bg_color = Color32::from_gray(43);
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.selection.bg_fill = Color32::from_rgb(39, 65, 87);
    style.visuals.selection.stroke = Stroke::new(1.0_f32, TEXT);
    style.visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    style.visuals.widgets.inactive.bg_fill = SURFACE;
    style.visuals.widgets.inactive.weak_bg_fill = SURFACE;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    style.visuals.widgets.hovered.bg_fill = HOVER;
    style.visuals.widgets.hovered.weak_bg_fill = HOVER;
    style.visuals.widgets.active.bg_fill = Color32::from_gray(53);
    style.spacing.item_spacing = egui::vec2(10., 8.);
    style.spacing.button_padding = egui::vec2(10., 6.);
    style.spacing.interact_size.y = 28.;
    ctx.set_style(style);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn charcoal_surfaces_are_neutral_and_applied_to_widgets() {
        for color in [DEEP, EDITOR, SURFACE, HOVER, BORDER] {
            assert_eq!(color.r(), color.g());
            assert_eq!(color.g(), color.b());
        }
        assert!(DEEP.r() < EDITOR.r() && EDITOR.r() < SURFACE.r());
        let context = egui::Context::default();
        apply(&context);
        let style = context.style();
        assert_eq!(style.visuals.panel_fill, DEEP);
        assert_eq!(style.visuals.extreme_bg_color, EDITOR);
        assert_eq!(style.visuals.widgets.inactive.bg_fill, SURFACE);
        assert_eq!(style.visuals.override_text_color, Some(TEXT));
    }
}
