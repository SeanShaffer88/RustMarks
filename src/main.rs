#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod editor;
mod merge_workspace;
mod tessel;
mod ui_components;
fn main() -> eframe::Result<()> {
    // Any argument at all selects the headless CLI (see rustmarks::cli);
    // no arguments opens the window.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        let (code, output) = rustmarks::cli::run(&args);
        print!("{output}");
        std::process::exit(code);
    }
    let state = rustmarks::window_state::WindowState::load(
        &rustmarks::window_state::WindowState::default_path(),
    );
    let mut viewport = egui::ViewportBuilder::default()
        .with_icon(
            eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon-256.png"))
                .expect("assets/icon-256.png is a valid PNG"),
        )
        .with_inner_size(state.size)
        .with_min_inner_size(rustmarks::window_state::MIN_SIZE)
        .with_maximized(state.maximized);
    if let Some(p) = state.position {
        viewport = viewport.with_position(p);
    }
    let settings = rustmarks::ui_settings::UiSettings::load();
    eframe::run_native(
        "RustMarks — Bookmark Studio",
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(move |cc| {
            tessel::install_fonts(&cc.egui_ctx);
            tessel::apply(&cc.egui_ctx, &settings);
            Ok(Box::new(editor::Studio::with_ui_settings(settings)))
        }),
    )
}
