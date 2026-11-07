use crate::vscode_theme as theme;
use egui::{Color32, RichText, Stroke, Ui};

pub fn card<R>(ui: &mut Ui, body: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::none()
        .fill(theme::SURFACE)
        .stroke(Stroke::new(1.0_f32, theme::BORDER))
        .rounding(6.0_f32)
        .inner_margin(16.0_f32)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            body(ui)
        })
        .inner
}
pub fn section(ui: &mut Ui, label: &str) {
    ui.add_space(8.);
    ui.label(RichText::new(label).strong().size(15.));
    ui.add_space(2.);
}
pub fn metric(ui: &mut Ui, count: usize, label: &str, color: Color32) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(7., 7.), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 3., color);
        ui.label(
            RichText::new(count.to_string())
                .size(21.)
                .color(theme::TEXT),
        );
    });
    ui.label(RichText::new(label).size(11.).color(theme::MUTED));
}
pub fn primary(ui: &mut Ui, text: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(text).strong().color(Color32::WHITE))
            .fill(theme::ACCENT)
            .rounding(5.0_f32)
            .min_size(egui::vec2(165., 36.)),
    )
}
pub struct SourceCard<'a> {
    pub label: &'a str,
    pub name: &'a str,
    pub path: &'a str,
    pub count: usize,
    pub action: &'a str,
}
pub fn source_card(ui: &mut Ui, source: SourceCard<'_>) -> bool {
    card(ui, |ui| {
        let mut clicked = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new(source.label).small().color(theme::MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                clicked = ui
                    .small_button("···")
                    .on_hover_text(source.action)
                    .clicked();
            });
        });
        ui.add_space(3.);
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(27., 32.), egui::Sense::hover());
            let p = ui.painter();
            p.rect_filled(rect.shrink(2.), 4.0_f32, Color32::from_rgb(48, 75, 96));
            for y in [10., 16., 22.] {
                p.line_segment(
                    [rect.min + egui::vec2(7., y), rect.min + egui::vec2(20., y)],
                    Stroke::new(1.2_f32, Color32::from_rgb(140, 191, 230)),
                );
            }
            ui.vertical(|ui| {
                ui.add(egui::Label::new(RichText::new(source.name).strong()).truncate())
                    .on_hover_text(source.name);
                ui.add(
                    egui::Label::new(RichText::new(source.path).small().color(theme::MUTED))
                        .truncate(),
                )
                .on_hover_text(source.path);
            });
        });
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} bookmarks", source.count))
                    .small()
                    .color(theme::MUTED),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                clicked |= ui.small_button(source.action).clicked();
            });
        });
        clicked
    })
}
#[derive(Clone, Copy)]
pub enum Icon {
    Library,
    Merge,
    Compare,
    Organize,
    History,
    Settings,
}
fn draw_icon(ui: &Ui, center: egui::Pos2, icon: Icon, color: Color32) {
    let p = ui.painter();
    let s = Stroke::new(1.4_f32, color);
    let at = |x, y| center + egui::vec2(x, y);
    match icon {
        Icon::Library => {
            p.rect_stroke(
                egui::Rect::from_center_size(center, egui::vec2(12., 14.)),
                2.0_f32,
                s,
            );
            for y in [-3., 1., 5.] {
                p.line_segment([at(-3., y), at(3., y)], s);
            }
        }
        Icon::Merge => {
            for (x, y, d) in [(0., -4., 1.), (0., 4., -1.)] {
                p.line_segment([at(x - 6., y), at(x + 6., y)], s);
                p.line_segment([at(x + 3. * d, y - 3.), at(x + 6. * d, y)], s);
                p.line_segment([at(x + 3. * d, y + 3.), at(x + 6. * d, y)], s);
            }
        }
        Icon::Compare => {
            for x in [-4., 4.] {
                p.rect_stroke(
                    egui::Rect::from_center_size(at(x, 0.), egui::vec2(6., 13.)),
                    1.0_f32,
                    s,
                );
            }
        }
        Icon::Organize => {
            for x in [-4., 4.] {
                for y in [-4., 4.] {
                    p.rect_stroke(
                        egui::Rect::from_center_size(at(x, y), egui::vec2(5., 5.)),
                        1.0_f32,
                        s,
                    );
                }
            }
        }
        Icon::History => {
            p.circle_stroke(center, 7., s);
            p.line_segment([at(0., -4.), center], s);
            p.line_segment([center, at(4., 2.)], s);
        }
        Icon::Settings => {
            p.circle_stroke(center, 5., s);
            p.circle_stroke(center, 1.5, s);
            for (x, y) in [(0., 1.), (1., 0.), (0., -1.), (-1., 0.)] {
                p.line_segment([at(x * 5., y * 5.), at(x * 8., y * 8.)], s);
            }
        }
    }
}
pub fn navigation(ui: &mut Ui, icon: Icon, text: &str, selected: bool) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text));
    if selected || response.hovered() || response.has_focus() {
        ui.painter().rect_filled(
            rect,
            4.0_f32,
            if selected {
                Color32::from_rgb(38, 55, 70)
            } else {
                theme::HOVER
            },
        );
    }
    if selected {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(rect.min + egui::vec2(0., 9.), egui::vec2(3., 20.)),
            2.0_f32,
            theme::ACCENT,
        );
    }
    let color = if selected { theme::TEXT } else { theme::MUTED };
    draw_icon(
        ui,
        egui::pos2(rect.left() + 21., rect.center().y),
        icon,
        if selected { theme::ACCENT } else { color },
    );
    ui.painter().text(
        egui::pos2(rect.left() + 40., rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        egui::FontId::proportional(13.),
        color,
    );
    response
}
