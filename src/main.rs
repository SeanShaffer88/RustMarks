#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod editor;
mod merge_workspace;
mod ui_components;
mod vscode_theme;
fn main() -> eframe::Result<()> {
    eframe::run_native(
        "RustMarks — Bookmark Studio",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1440., 900.])
                .with_min_inner_size([1000., 650.]),
            ..Default::default()
        },
        Box::new(|cc| {
            vscode_theme::apply(&cc.egui_ctx);
            Ok(Box::new(editor::Studio::default()))
        }),
    )
}
