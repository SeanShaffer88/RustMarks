#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod editor;
mod merge_workspace;
mod ui_components;
mod vscode_theme;
fn main() -> eframe::Result<()> {
    let state = rustmarks::window_state::WindowState::load(
        &rustmarks::window_state::WindowState::default_path(),
    );
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size(state.size)
        .with_min_inner_size(rustmarks::window_state::MIN_SIZE)
        .with_maximized(state.maximized);
    if let Some(p) = state.position {
        viewport = viewport.with_position(p);
    }
    eframe::run_native(
        "RustMarks — Bookmark Studio",
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(|cc| {
            vscode_theme::apply(&cc.egui_ctx);
            Ok(Box::new(editor::Studio::default()))
        }),
    )
}
