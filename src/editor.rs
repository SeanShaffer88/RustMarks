use crate::{
    merge_workspace::{Action as MergeAction, MergeWorkspace},
    tessel as theme, ui_components as components,
};
use egui::{Color32, RichText};
use rustmarks::{
    diff_view,
    grid::{self, Direction},
    heuristics,
    history::{History, Snapshot},
    merge::MergeSession,
    models::*,
    parser,
    rules::RuleSet,
    ui_settings::{AccentPreset, Density, ThemeMode, UiSettings},
    undo::UndoHistory,
};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
use uuid::Uuid;
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum View {
    Explorer,
    Organizer,
    Merge,
    History,
    Health,
    Settings,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Grid,
    Details,
    Diff,
}
pub struct Studio {
    doc: BookmarkDocument,
    original: BookmarkDocument,
    view: View,
    tab: Tab,
    query: String,
    folder: Option<Uuid>,
    selected: HashSet<Uuid>,
    anchor: Option<usize>,
    bulk_tag: String,
    message: String,
    history: History,
    snapshots: Vec<Snapshot>,
    merge_session: Option<MergeSession>,
    merge_ui: MergeWorkspace,
    unified: bool,
    confirm_discard: bool,
    pending_open: bool,
    pending_drop: Option<PathBuf>,
    confirm_close: bool,
    rules: RuleSet,
    rules_text: String,
    undo: UndoHistory,
    checkpoint: BookmarkDocument,
    last_edit: Option<std::time::Instant>,
    navigating_history: bool,
    cell_focus: Option<(Uuid, usize)>,
    focus_request: Option<(Uuid, usize)>,
    icon_texture: Option<(String, egui::TextureHandle)>,
    diff_collapsed: HashSet<Uuid>,
    move_target: Option<Uuid>,
    inspected_snapshot: Option<(Snapshot, BookmarkDocument)>,
    imported_browser_json: bool,
    source_revision: Option<rustmarks::revision::FileRevision>,
    window_saved: Option<rustmarks::window_state::WindowState>,
    window_pending: Option<(rustmarks::window_state::WindowState, std::time::Instant)>,
    ui_settings: UiSettings,
    health: Option<rustmarks::health::HealthReport>,
    /// Bumped once per frame in which the document changed (detected by
    /// the checkpoint sync in update); drives the grid caches below.
    doc_revision: u64,
    rows_cache: Option<(u64, Option<Uuid>, String, Vec<BookmarkItem>)>,
    folders_cache: Option<(u64, Vec<(Uuid, String)>)>,
    count_cache: Option<(u64, usize)>,
    /// Some(name draft) while the user names a search to save.
    search_name: Option<String>,
    /// Watch-folder state: file mtimes at the last scan, whether the
    /// silent baseline scan has run, and a detected new/changed export.
    watch_seen: std::collections::HashMap<PathBuf, std::time::SystemTime>,
    watch_baseline: bool,
    watch_found: Option<PathBuf>,
    watch_last_check: Option<std::time::Instant>,
    palette_open: bool,
    palette_query: String,
    palette_index: usize,
    palette_focus: bool,
}
impl Default for Studio {
    fn default() -> Self {
        let doc = BookmarkDocument::default();
        let history = History::default();
        let rules_path = history
            .directory
            .parent()
            .unwrap_or(&history.directory)
            .join("rules.json");
        let loaded_rules = if rules_path.exists() {
            std::fs::read_to_string(&rules_path)
                .map_err(|e| e.to_string())
                .and_then(|s| RuleSet::parse(&s).map_err(|e| e.to_string()))
        } else {
            Ok(RuleSet::default())
        };
        let message = loaded_rules
            .as_ref()
            .err()
            .map(|e| format!("Custom rules could not be loaded; using defaults: {e}"))
            .unwrap_or_else(|| "Ready. Open a bookmark file to get started.".into());
        let rules = loaded_rules.unwrap_or_default();
        let rules_text = serde_json::to_string_pretty(&rules).unwrap_or_default();
        Self {
            original: doc.clone(),
            checkpoint: doc.clone(),
            doc,
            view: View::Explorer,
            tab: Tab::Grid,
            query: String::new(),
            folder: None,
            selected: HashSet::new(),
            anchor: None,
            bulk_tag: String::new(),
            message,
            history: History::default(),
            snapshots: vec![],
            merge_session: None,
            merge_ui: MergeWorkspace::default(),
            unified: false,
            confirm_discard: false,
            pending_open: false,
            pending_drop: None,
            confirm_close: false,
            rules,
            rules_text,
            undo: UndoHistory::default(),
            last_edit: None,
            navigating_history: false,
            cell_focus: None,
            focus_request: None,
            icon_texture: None,
            diff_collapsed: HashSet::new(),
            move_target: None,
            inspected_snapshot: None,
            imported_browser_json: false,
            source_revision: None,
            window_saved: None,
            window_pending: None,
            ui_settings: UiSettings::default(),
            health: None,
            doc_revision: 0,
            rows_cache: None,
            folders_cache: None,
            count_cache: None,
            search_name: None,
            watch_seen: std::collections::HashMap::new(),
            watch_baseline: false,
            watch_found: None,
            watch_last_check: None,
            palette_open: false,
            palette_query: String::new(),
            palette_index: 0,
            palette_focus: false,
        }
    }
}
impl Studio {
    /// A studio whose appearance starts from persisted settings (used by
    /// `main`); `Studio::default()` keeps the Tessel defaults for tests.
    pub fn with_ui_settings(ui_settings: UiSettings) -> Self {
        Self {
            ui_settings,
            ..Self::default()
        }
    }

    /// Apply the current appearance settings to the context and persist
    /// them. Called when the user changes theme, accent or density.
    fn appearance_changed(&mut self, ctx: &egui::Context) {
        theme::apply(ctx, &self.ui_settings);
        if let Err(error) = self.ui_settings.save() {
            self.message = format!("Appearance applied but could not be saved: {error}");
        }
    }

    /// Debounced, atomic persistence of window size/position/maximized state.
    fn persist_window_state(&mut self, ctx: &egui::Context) {
        use rustmarks::window_state::WindowState;
        const DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(600);
        let (inner, outer, maximized, minimized) = ctx.input(|i| {
            let v = i.viewport();
            (v.inner_rect, v.outer_rect, v.maximized, v.minimized)
        });
        let (Some(inner), false) = (inner, minimized.unwrap_or(false)) else {
            return;
        };
        let maximized = maximized.unwrap_or(false);
        let prior = self.window_saved.unwrap_or_default();
        // While maximized keep the restored size/position that was last saved.
        let current = if maximized {
            WindowState { maximized, ..prior }
        } else {
            WindowState {
                size: [inner.width(), inner.height()],
                position: outer.map(|r| [r.min.x, r.min.y]),
                maximized,
            }
            .sanitized()
        };
        if self.window_saved.is_none() {
            self.window_saved = Some(current);
            return;
        }
        if self.window_saved == Some(current) {
            self.window_pending = None;
            return;
        }
        let since = match self.window_pending {
            Some((p, t)) if p == current => t,
            _ => {
                let now = std::time::Instant::now();
                self.window_pending = Some((current, now));
                now
            }
        };
        if since.elapsed() >= DEBOUNCE {
            if current.save(&WindowState::default_path()).is_ok() {
                self.window_saved = Some(current);
            }
            self.window_pending = None;
        } else {
            ctx.request_repaint_after(DEBOUNCE);
        }
    }

    fn workspace(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("titlebar").show(ctx, |ui| self.toolbar(ui));
        if let Some(found) = self.watch_found.clone() {
            egui::TopBottomPanel::top("watch_banner")
                .frame(
                    egui::Frame::none()
                        .fill(theme::colors().raised)
                        .inner_margin(egui::Margin::symmetric(12., 6.)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "New bookmark export detected: {}",
                            found
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_else(|| found.display().to_string())
                        ));
                        if components::primary_compact(ui, "Merge it", true)
                            .on_hover_text(
                                "Stage this file as a merge against the open library, ready for review.",
                            )
                            .clicked()
                        {
                            self.watch_found = None;
                            self.prepare_merge(vec![found.clone()]);
                        }
                        if ui
                            .button("Dismiss")
                            .on_hover_text("Ignore this file until it changes again.")
                            .clicked()
                        {
                            self.watch_found = None;
                        }
                    });
                });
        }
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::none()
                    .fill(theme::colors().background)
                    .inner_margin(egui::Margin::symmetric(12., 5.)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{}{}",
                            self.doc.source_path.as_deref().unwrap_or("Untitled"),
                            if self.dirty() { "  [Modified]" } else { "" }
                        ))
                        .color(Color32::WHITE),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "{} bookmarks  ·  Local rules active  ·  UTF-8",
                                self.bookmark_count()
                            ))
                            .color(Color32::WHITE),
                        );
                    });
                });
            });
        egui::TopBottomPanel::bottom("notification").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.message).small());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("Ctrl+K Commands · Ctrl+O Open · Ctrl+S Save · F1 Help")
                            .small()
                            .color(theme::colors().muted),
                    )
                    .on_hover_text("Keyboard shortcuts. The full list lives in Settings & help.");
                });
            });
        });
        egui::SidePanel::left("activity")
            .exact_width(174.)
            .resizable(false)
            .frame(
                egui::Frame::none()
                    .fill(theme::colors().background)
                    .inner_margin(10.0_f32),
            )
            .show(ctx, |ui| {
                ui.add_space(10.);
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(27., 27.), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, 6.0_f32, theme::colors().action);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "R",
                        egui::FontId::proportional(17.),
                        Color32::WHITE,
                    );
                    ui.vertical(|ui| {
                        ui.label(RichText::new("RustMarks").strong().size(15.));
                        ui.label(
                            RichText::new("BOOKMARK STUDIO")
                                .size(8.)
                                .color(theme::colors().muted),
                        );
                    });
                });
                ui.add_space(22.);
                use components::Icon;
                for (view, tab, icon, label) in [
                    (View::Explorer, Tab::Grid, Icon::Library, "Library"),
                    (View::Merge, Tab::Grid, Icon::Merge, "Merge"),
                    (View::Explorer, Tab::Diff, Icon::Compare, "Compare"),
                    (View::Organizer, Tab::Grid, Icon::Organize, "Organize"),
                    (View::Health, Tab::Grid, Icon::Health, "Health"),
                    (View::History, Tab::Grid, Icon::History, "History"),
                    (View::Settings, Tab::Grid, Icon::Settings, "Settings"),
                ] {
                    let selected = self.view == view
                        && (view != View::Explorer
                            || if tab == Tab::Diff {
                                self.tab == Tab::Diff
                            } else {
                                self.tab != Tab::Diff
                            });
                    let response = components::navigation(ui, icon, label, selected);
                    let keyboard = response.has_focus()
                        && ui.input_mut(|input| {
                            input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                                || input.consume_key(egui::Modifiers::NONE, egui::Key::Space)
                        });
                    if response.clicked() || keyboard {
                        self.navigate(view, tab);
                    }
                    ui.add_space(2.);
                }
                ui.add_space(24.);
                ui.separator();
                ui.label(
                    RichText::new("PRIVATE BY DESIGN")
                        .size(9.)
                        .color(theme::colors().muted),
                );
                ui.label(
                    RichText::new("Your bookmarks stay\non this device.")
                        .size(11.)
                        .color(theme::colors().muted),
                );
            });
        if self.view == View::Explorer && self.tab != Tab::Diff {
            egui::SidePanel::left("explorer")
                .default_width(220.)
                .width_range(180. ..=400.)
                .show(ctx, |ui| self.explorer(ui));
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(theme::colors().surface)
                    .inner_margin(24.0_f32),
            )
            .show(ctx, |ui| match self.view {
                View::Explorer => {
                    self.editor_tabs(ui);
                    theme::active_rule(ui);
                    match self.tab {
                        Tab::Grid => self.grid(ui),
                        Tab::Details => self.details(ui),
                        Tab::Diff => self.diff(ui),
                    }
                }
                View::Organizer => self.organizer(ui),
                View::Health => {
                    egui::ScrollArea::vertical().show(ui, |ui| self.health_view(ui));
                }
                View::Merge => self.merge_panel(ui),
                View::History => self.history_panel(ui),
                View::Settings => {
                    egui::ScrollArea::vertical().show(ui, |ui| self.settings(ui));
                }
            });
    }
    fn changes_tab_label(&self) -> &'static str {
        if self.dirty() {
            "⇄  Changes ●"
        } else {
            "⇄  Changes"
        }
    }
    fn editor_tabs(&mut self, ui: &mut egui::Ui) -> [egui::Response; 3] {
        // Compute document-derived labels before lending `self.tab` to a widget.
        // Doing both in selectable_value's arguments caused E0502 in Windows CI.
        let changes_label = self.changes_tab_label();
        ui.horizontal(|ui| {
            [
                ui.selectable_value(&mut self.tab, Tab::Grid, "▤  Bookmarks")
                    .on_hover_text("Edit bookmarks in the grid"),
                ui.selectable_value(&mut self.tab, Tab::Details, "≡  Details")
                    .on_hover_text("Edit notes, tags, URL and favicon of the selected bookmark"),
                ui.selectable_value(&mut self.tab, Tab::Diff, changes_label)
                    .on_hover_text("Review uncommitted changes before saving"),
            ]
        })
        .inner
    }
    fn undo_edit(&mut self) {
        let source_path = self.doc.source_path.clone();
        if self.undo.undo(&mut self.doc) {
            self.doc.source_path = source_path;
            self.checkpoint = self.doc.clone();
            self.last_edit = None;
            self.navigating_history = true;
            self.selected.clear();
            self.folder = None;
        }
    }
    fn redo_edit(&mut self) {
        let source_path = self.doc.source_path.clone();
        if self.undo.redo(&mut self.doc) {
            self.doc.source_path = source_path;
            self.checkpoint = self.doc.clone();
            self.last_edit = None;
            self.navigating_history = true;
            self.selected.clear();
            self.folder = None;
        }
    }

    fn dirty(&self) -> bool {
        self.doc != self.original
    }
    fn snapshot(&mut self, reason: &str) -> bool {
        match self.history.save(&self.doc, reason) {
            Ok(()) => {
                // Retention: keep only the newest N snapshots (Settings).
                let keep = self.ui_settings.snapshot_keep_last;
                if keep > 0 {
                    let _ = self.history.prune(keep as usize);
                }
                true
            }
            Err(e) => {
                self.message = format!("Snapshot failed; operation cancelled: {e}");
                false
            }
        }
    }
    fn open(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Bookmarks", &["html", "htm", "json"])
            .pick_file()
        {
            self.open_path(path);
        }
    }
    fn open_path(&mut self, path: PathBuf) {
        match parser::load_tracked(&path) {
            Ok(loaded) => {
                self.imported_browser_json = loaded.format == parser::SourceFormat::ChromeJson;
                self.source_revision = Some(loaded.revision);
                let doc = loaded.document;
                self.original = doc.clone();
                self.doc = doc;
                self.undo = UndoHistory::default();
                self.checkpoint = self.doc.clone();
                self.last_edit = None;
                self.cell_focus = None;
                self.merge_session = None;
                self.merge_ui.reset();
                self.folder = None;
                self.selected.clear();
                self.message = "File opened locally. No data leaves this device.".into();
            }
            Err(e) => self.message = format!("Open failed: {e}"),
        }
    }
    fn save(&mut self, save_as: bool) {
        let path = if save_as || self.doc.source_path.is_none() || self.imported_browser_json {
            rfd::FileDialog::new()
                .add_filter("RustMarks JSON (all metadata)", &["json"])
                .add_filter("Netscape bookmarks", &["html"])
                .set_file_name("bookmarks.json")
                .save_file()
        } else {
            self.doc.source_path.as_ref().map(PathBuf::from)
        };
        if let Some(path) = path {
            if self.imported_browser_json
                && self
                    .source_revision
                    .as_ref()
                    .is_some_and(|revision| revision.refers_to(&path))
            {
                self.message="Choose a new working file to preserve the original Chrome/Edge JSON. Use Settings to export browser JSON.".into();
                return;
            }
            if let Some(revision) = &self.source_revision {
                if revision.refers_to(&path) {
                    if let Err(error) = revision.ensure_unchanged() {
                        self.message = format!("Save cancelled: {error}");
                        return;
                    }
                }
            }
            if !self.snapshot("before-save") {
                return;
            }
            // Preserve the previous committed state as well as the staged state.
            if let Err(e) = self.history.save(&self.original, "committed") {
                self.message = format!("Backup failed: {e}");
                return;
            }
            let mut doc = self.doc.clone();
            doc.source_path = Some(path.display().to_string());
            match parser::save_checked(&doc, &path, self.source_revision.as_ref()) {
                Ok(revision) => {
                    self.source_revision = Some(revision);
                    self.checkpoint.source_path = doc.source_path.clone();
                    self.imported_browser_json = false;
                    self.doc = doc.clone();
                    self.original = doc;
                    self.message = format!("Saved {}", path.display());
                }
                Err(e) => self.message = format!("Save failed: {e}"),
            }
        }
    }
    fn add_bookmark(&mut self) {
        if self.snapshot("add") {
            let b = BookmarkItem::new("Untitled bookmark", "https://");
            self.selected.clear();
            self.selected.insert(b.id);
            self.doc.add(self.folder, BookmarkNode::Bookmark(b));
        }
    }
    fn export_chrome_json(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Chrome JSON", &["json"])
            .set_file_name("bookmarks-chrome.json")
            .save_file()
        {
            match serde_json::to_vec_pretty(&parser::chrome(&self.doc)) {
                Ok(data) => match rustmarks::storage::atomic_write(&path, &data) {
                    Ok(()) => self.message = format!("Exported {}", path.display()),
                    Err(e) => self.message = e.to_string(),
                },
                Err(e) => self.message = e.to_string(),
            }
        }
    }
    fn prepare_merge(&mut self, paths: Vec<PathBuf>) {
        let inputs: parser::Result<Vec<_>> = paths
            .into_iter()
            .map(|path| parser::load(&path).map(|document| (path.display().to_string(), document)))
            .collect();
        match inputs.and_then(|inputs| MergeSession::new(&self.doc, inputs)) {
            Ok(session) => {
                self.merge_session = Some(session);
                self.merge_ui.reset();
                self.view = View::Merge;
                self.message =
                    "Preview ready. Choose how to resolve each conflict, then review changes."
                        .into();
            }
            Err(error) => {
                self.message = format!(
                    "Merge import failed; library and previous preview are unchanged: {error}"
                )
            }
        }
    }
    fn begin_merge(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Bookmarks", &["html", "htm", "json"])
            .pick_files()
        {
            self.prepare_merge(paths);
        }
    }
    fn refresh_merge(&mut self) {
        if let Some(session) = &self.merge_session {
            let paths = session
                .sources()
                .iter()
                .map(|source| PathBuf::from(&source.path))
                .collect();
            self.prepare_merge(paths);
        }
    }
    fn stage_merge(&mut self) {
        let candidate = match self.merge_session.as_ref() {
            Some(session) => session.apply(&self.doc),
            None => {
                self.message = "Choose incoming files to build a preview first.".into();
                return;
            }
        };
        match candidate {
            Ok(document) => {
                if self.snapshot("before-merge") {
                    self.doc = document;
                    self.merge_session = None;
                    self.merge_ui.reset();
                    self.folder = None;
                    self.selected.clear();
                    self.view = View::Explorer;
                    self.tab = Tab::Diff;
                    self.message =
                        "Merge staged. Review the differences, then Save to commit to disk.".into();
                }
            }
            Err(error) => self.message = format!("Merge not applied: {error}"),
        }
    }
    fn request_open(&mut self) {
        if self.dirty() {
            self.pending_open = true;
            self.confirm_discard = true;
        } else {
            self.open();
        }
    }
    fn navigate(&mut self, view: View, tab: Tab) {
        self.view = view;
        if view == View::Explorer {
            self.tab = tab;
        }
        if view == View::History {
            match self.history.list() {
                Ok(snapshots) => self.snapshots = snapshots,
                Err(error) => self.message = error.to_string(),
            }
        }
    }
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open bookmarks…       Ctrl+O").on_hover_text("Open a bookmark file (HTML, Chrome/Edge JSON or RustMarks JSON). Prompts if there are unsaved changes.").clicked() {
                    ui.close_menu();
                    self.request_open();
                }
                if ui.button("Save                         Ctrl+S").on_hover_text("Save to the opened file. Refuses if the file changed on disk; a safety snapshot is taken first.").clicked() {
                    ui.close_menu();
                    self.save(false);
                }
                if ui.button("Save as…       Ctrl+Shift+S").on_hover_text("Save to a new file. The original file is left untouched.").clicked() {
                    ui.close_menu();
                    self.save(true);
                }
                ui.separator();
                if ui.button("Merge files…").on_hover_text("Choose files to merge into this library. Opens the Merge view; nothing is written to disk.").clicked() {
                    ui.close_menu();
                    self.begin_merge();
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui
                    .add_enabled(
                        self.undo.can_undo(),
                        egui::Button::new("Undo document edit"),
                    )
                    .on_hover_text("Undo the last document edit · Ctrl+Z")
                    .clicked()
                {
                    self.undo_edit();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(
                        self.undo.can_redo(),
                        egui::Button::new("Redo document edit"),
                    )
                    .on_hover_text("Redo the last undone edit · Ctrl+Y")
                    .clicked()
                {
                    self.redo_edit();
                    ui.close_menu();
                }
            });
            ui.menu_button("View", |ui| {
                for (label, view, tab) in [
                    ("Library", View::Explorer, Tab::Grid),
                    ("Merge", View::Merge, Tab::Grid),
                    ("Compare", View::Explorer, Tab::Diff),
                    ("Organize", View::Organizer, Tab::Grid),
                    ("Health", View::Health, Tab::Grid),
                    ("History", View::History, Tab::Grid),
                    ("Settings", View::Settings, Tab::Grid),
                ] {
                    if ui
                        .button(label)
                        .on_hover_text(format!("Switch to the {label} view"))
                        .clicked()
                    {
                        self.navigate(view, tab);
                        ui.close_menu();
                    }
                }
            });
            ui.menu_button("Tools", |ui| {
                if ui.button("Scan categorization suggestions").on_hover_text("Scan bookmarks with the local rules and propose folders. Does not move anything.").clicked() {
                    if self.snapshot("suggest") {
                        heuristics::suggest_with(&mut self.doc, &self.rules);
                        self.view = View::Organizer;
                    }
                    ui.close_menu();
                }
            });
            ui.menu_button("Help", |ui| {
                if ui
                    .button("Settings & keyboard shortcuts")
                    .on_hover_text("Open settings, shortcuts and the categorization rules editor.")
                    .clicked()
                {
                    self.view = View::Settings;
                    ui.close_menu();
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new("●  Local workspace")
                        .size(11.)
                        .color(theme::colors().success),
                );
                if components::primary_compact(ui, "Save", true)
                    .on_hover_text("Commit staged changes · Ctrl+S")
                    .clicked()
                {
                    self.save(false);
                }
                if ui
                    .small_button("Open")
                    .on_hover_text("Open a bookmark file. Prompts if there are unsaved changes.")
                    .clicked()
                {
                    self.request_open();
                }
            });
        });
    }
    fn explorer(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("EXPLORER")
                .small()
                .color(theme::colors().muted),
        );
        ui.separator();
        ui.add(
            egui::TextEdit::singleline(&mut self.query)
                .hint_text("Search · tag:dev -broken")
                .desired_width(f32::INFINITY),
        )
        .on_hover_text("Search titles, URLs, tags and notes. Use tag:name and -word to exclude.");
        // Saved searches: save the current query under a name, reapply or
        // delete saved ones. Persisted in settings.json.
        if self.search_name.is_some() {
            let mut name = self.search_name.clone().unwrap_or_default();
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut name)
                        .hint_text("Name this search")
                        .desired_width(120.),
                );
                if ui
                    .button("Save")
                    .on_hover_text("Save the current search text under this name.")
                    .clicked()
                    && !name.trim().is_empty()
                {
                    self.ui_settings
                        .saved_searches
                        .push(rustmarks::ui_settings::SavedSearch {
                            name: name.trim().to_owned(),
                            query: self.query.clone(),
                        });
                    if let Err(e) = self.ui_settings.save() {
                        self.message = e.to_string();
                    }
                    self.search_name = None;
                }
                if ui
                    .button("Cancel")
                    .on_hover_text("Close without saving.")
                    .clicked()
                {
                    self.search_name = None;
                }
            });
            if self.search_name.is_some() {
                self.search_name = Some(name);
            }
        } else if !self.query.trim().is_empty()
            && ui
                .button("Save this search…")
                .on_hover_text("Save the current search so you can reapply it in one click.")
                .clicked()
        {
            self.search_name = Some(String::new());
        }
        if !self.ui_settings.saved_searches.is_empty() {
            let searches = self.ui_settings.saved_searches.clone();
            let mut apply: Option<String> = None;
            let mut remove: Option<usize> = None;
            ui.horizontal_wrapped(|ui| {
                for (index, saved) in searches.iter().enumerate() {
                    if ui
                        .button(&saved.name)
                        .on_hover_text(format!("Search for: {}", saved.query))
                        .clicked()
                    {
                        apply = Some(saved.query.clone());
                    }
                    if ui
                        .small_button("×")
                        .on_hover_text(format!(
                            "Delete the saved search \u{201c}{}\u{201d}",
                            saved.name
                        ))
                        .clicked()
                    {
                        remove = Some(index);
                    }
                }
            });
            if let Some(query) = apply {
                self.query = query;
            }
            if let Some(index) = remove {
                self.ui_settings.saved_searches.remove(index);
                if let Err(e) = self.ui_settings.save() {
                    self.message = e.to_string();
                }
            }
        }
        if ui
            .selectable_label(
                self.folder.is_none(),
                format!("All bookmarks  ({})", self.bookmark_count()),
            )
            .on_hover_text("Show bookmarks from every folder")
            .clicked()
        {
            self.folder = None;
        }
        fn tree(ui: &mut egui::Ui, n: &BookmarkNode, selected: &mut Option<Uuid>) {
            if let BookmarkNode::Folder {
                id,
                title,
                children,
                ..
            } = n
            {
                egui::CollapsingHeader::new(format!("▸ {title}"))
                    .id_source(id)
                    .default_open(true)
                    .show(ui, |ui| {
                        if ui
                            .selectable_label(*selected == Some(*id), "Folder contents")
                            .on_hover_text("Show only this folder's bookmarks")
                            .clicked()
                        {
                            *selected = Some(*id);
                        }
                        for child in children {
                            tree(ui, child, selected);
                        }
                    });
            }
        }
        egui::ScrollArea::vertical()
            .show(ui, |ui| tree(ui, &self.doc.root_folder, &mut self.folder));
        ui.separator();
        if let Some(id) = self.folder {
            if id != self.doc.root_folder.id() {
                if let Some(BookmarkNode::Folder { title, .. }) = self.doc.root_folder.find_mut(id)
                {
                    ui.label("Folder name");
                    ui.text_edit_singleline(title).on_hover_text("Folder name");
                }
                if components::danger(ui, "Delete folder and contents", true).on_hover_text("Delete this folder and every bookmark in it. A snapshot is taken first; undo with Ctrl+Z.").clicked()
                    && self.snapshot("delete-folder")
                {
                    let mut ids = HashSet::new();
                    ids.insert(id);
                    self.doc.root_folder.remove(&ids);
                    self.folder = None;
                    self.selected.clear();
                }
            }
        }
        if ui
            .button("+ New folder")
            .on_hover_text("Add an empty folder under the current one. A snapshot is taken first.")
            .clicked()
            && self.snapshot("new-folder")
        {
            self.doc
                .add(self.folder, BookmarkNode::folder("New folder"));
        }
        ui.label(
            RichText::new("Changes are staged until you save.\nReview them in the Changes tab.")
                .small()
                .color(theme::colors().muted),
        );
    }
    fn grid(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if components::primary_compact(ui, "+ Bookmark", true)
                .on_hover_text(
                    "Add a blank bookmark to the current folder. A snapshot is taken first.",
                )
                .clicked()
            {
                self.add_bookmark();
            }
            ui.label(format!("{} selected", self.selected.len()));
            if ui
                .button("Select all filtered")
                .on_hover_text("Select every bookmark matching the current search.")
                .clicked()
            {
                self.selected = self.visible().iter().map(|b| b.id).collect();
            }
            if components::danger(ui, "Delete selected", !self.selected.is_empty())
                .on_hover_text(
                    "Delete the selected bookmarks. A snapshot is taken first; undo with Ctrl+Z.",
                )
                .clicked()
                && self.snapshot("delete")
            {
                self.doc.root_folder.remove(&self.selected);
                self.selected.clear();
            }
        });
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.bulk_tag)
                    .hint_text("Tag for selected bookmarks")
                    .desired_width(220.),
            )
            .on_hover_text("Tag to add to every selected bookmark");
            if ui.button("Apply tag").on_hover_text("Add the typed tag to every selected bookmark.").clicked()
                && !self.bulk_tag.trim().is_empty()
                && !self.selected.is_empty()
                && self.snapshot("bulk-tag")
            {
                let tag = self.bulk_tag.trim().to_owned();
                let selected = &self.selected;
                self.doc.root_folder.visit_mut(&mut |b| {
                    if selected.contains(&b.id) && !b.tags.contains(&tag) {
                        b.tags.push(tag.clone());
                    }
                });
            }
            if ui.button("Clean tracking URLs").on_hover_text("Remove utm_*, fbclid, gclid and msclkid parameters from selected URLs. Signed URLs are left alone.").clicked()
                && !self.selected.is_empty()
                && self.snapshot("clean-urls")
            {
                let selected = &self.selected;
                self.doc.root_folder.visit_mut(&mut |b| {
                    if selected.contains(&b.id) {
                        b.url = rustmarks::url_cleanup::clean(&b.url)
                    }
                });
            }
        });
        ui.horizontal(|ui| {
            let folders = self.folder_list();
            let label = folders
                .iter()
                .find(|(id, _)| Some(*id) == self.move_target)
                .map(|(_, path)| path.as_str())
                .unwrap_or("Choose destination folder");
            egui::ComboBox::from_id_source("move-target")
                .selected_text(label)
                .show_ui(ui, |ui| {
                    for (id, path) in &folders {
                        ui.selectable_value(&mut self.move_target, Some(*id), path);
                    }
                })
                .response
                .on_hover_text("Destination folder for the selected bookmarks");
            if ui
                .add_enabled(
                    self.move_target.is_some() && !self.selected.is_empty(),
                    egui::Button::new("Move selected"),
                )
                .on_hover_text("Move the selected bookmarks to the chosen folder; identity is preserved. A snapshot is taken first.")
                .clicked()
                && self.snapshot("move")
            {
                if let Some(target) = self.move_target {
                    if let Err(e) = self.doc.move_bookmarks(&self.selected, target) {
                        self.message = e.to_string();
                    }
                }
            }
        });
        ui.separator();
        let rows = self.visible();
        let ids: Vec<_> = rows.iter().map(|b| b.id).collect();
        if rows.is_empty() {
            // Tessel empty state: the 24px grid motif behind a short message
            // and the primary action — never a blank area.
            let canvas = ui.available_rect_before_wrap();
            theme::paint_grid(ui, canvas);
            ui.add_space(60.);
            ui.vertical_centered(|ui| {
                ui.heading("A little order for your internet.");
                ui.label("Open an HTML or JSON bookmark file, or add your first bookmark.");
            });
            return;
        }
        let mut scroll_to = None;
        if let Some((id, col)) = self.cell_focus {
            if ui.memory(|m| m.has_focus(egui::Id::new(("cell", id, col)))) {
                if let Some(row) = ids.iter().position(|i| *i == id) {
                    let direction = ui.input_mut(|i| {
                        if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                            Some(Direction::Up)
                        } else if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
                            || i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                        {
                            Some(Direction::Down)
                        } else if i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab) {
                            Some(Direction::Previous)
                        } else if i.consume_key(egui::Modifiers::NONE, egui::Key::Tab) {
                            Some(Direction::Next)
                        } else {
                            None
                        }
                    });
                    if let Some(direction) = direction {
                        if let Some((r, c)) = grid::navigate(row, col, rows.len(), 4, direction) {
                            self.focus_request = Some((ids[r], c));
                            scroll_to = Some(r);
                        }
                    }
                }
            }
        }
        egui::ScrollArea::horizontal().show(ui, |ui| {
            use egui_extras::{Column, TableBuilder};
            let mut table = TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .column(Column::exact(45.))
                .column(Column::initial(190.).at_least(100.))
                .column(Column::initial(260.).at_least(120.))
                .column(Column::initial(140.).at_least(80.))
                .column(Column::initial(180.).at_least(100.))
                .column(Column::initial(160.).at_least(100.));
            if let Some(row) = scroll_to {
                table = table.scroll_to_row(row, Some(egui::Align::Center));
            }
            table
                .header(24., |mut header| {
                    for label in ["#", "TITLE", "URL", "TAGS", "NOTES", "CATEGORY"] {
                        header.col(|ui| {
                            ui.label(RichText::new(label).small().color(theme::colors().muted));
                        });
                    }
                })
                .body(|body| {
                    body.rows(theme::metrics().row_height, rows.len(), |mut row| {
                        let index = row.index();
                        let mut bookmark = rows[index].clone();
                        let before = bookmark.clone();
                        row.set_selected(self.selected.contains(&bookmark.id));
                        row.col(|ui| {
                            let selected = self.selected.contains(&bookmark.id);
                            if ui
                                .selectable_label(selected, format!("{:02}", index + 1))
                                .on_hover_text("Click to select; Ctrl-click toggles; Shift-click selects a range")
                                .clicked()
                            {
                                let modifiers = ui.input(|i| i.modifiers);
                                if modifiers.shift {
                                    let start = self.anchor.unwrap_or(index).min(ids.len() - 1);
                                    self.selected.extend(
                                        ids[start.min(index)..=start.max(index)].iter().copied(),
                                    );
                                } else if modifiers.ctrl || modifiers.command {
                                    if selected {
                                        self.selected.remove(&bookmark.id);
                                    } else {
                                        self.selected.insert(bookmark.id);
                                    }
                                    self.anchor = Some(index);
                                } else {
                                    self.selected.clear();
                                    self.selected.insert(bookmark.id);
                                    self.anchor = Some(index);
                                }
                            }
                        });
                        let mut tags = bookmark.tags.join(", ");
                        for col in 0..4 {
                            row.col(|ui| {
                                let value = match col {
                                    0 => &mut bookmark.title,
                                    1 => &mut bookmark.url,
                                    2 => &mut tags,
                                    _ => &mut bookmark.notes,
                                };
                                let response = ui.add(
                                    egui::TextEdit::singleline(value)
                                        .id(egui::Id::new(("cell", bookmark.id, col)))
                                        .font(egui::FontId::proportional(13.))
                                        .desired_width(f32::INFINITY),
                                )
                                .on_hover_text("Edit this cell. Enter/Up/Down move rows, Tab moves cells, Ctrl+Z undoes.");
                                if self.focus_request == Some((bookmark.id, col)) {
                                    response.request_focus();
                                    self.focus_request = None;
                                }
                                if response.has_focus() {
                                    self.cell_focus = Some((bookmark.id, col));
                                }
                            });
                        }
                        if tags != bookmark.tags.join(", ") {
                            bookmark.tags = tags
                                .split(',')
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                                .map(str::to_owned)
                                .collect();
                        }
                        row.col(|ui| {
                            ui.colored_label(
                                theme::colors().success,
                                bookmark.category_guess.as_deref().unwrap_or("—"),
                            );
                        });
                        if bookmark != before {
                            if let Some(BookmarkNode::Bookmark(target)) =
                                self.doc.root_folder.find_mut(bookmark.id)
                            {
                                *target = bookmark;
                            }
                        }
                    });
                });
        });
    }
    /// The grid's rows, cached: recomputed only when the document, the
    /// selected folder or the search text changed, so large libraries
    /// stay responsive while typing or scrolling.
    fn visible(&mut self) -> Vec<BookmarkItem> {
        if let Some((revision, folder, query, rows)) = &self.rows_cache {
            if *revision == self.doc_revision && *folder == self.folder && query == &self.query {
                return rows.clone();
            }
        }
        let rows = rustmarks::grid::visible_bookmarks(&self.doc, self.folder, &self.query);
        self.rows_cache = Some((
            self.doc_revision,
            self.folder,
            self.query.clone(),
            rows.clone(),
        ));
        rows
    }
    fn bookmark_count(&mut self) -> usize {
        if let Some((revision, count)) = self.count_cache {
            if revision == self.doc_revision {
                return count;
            }
        }
        let count = self.doc.items().len();
        self.count_cache = Some((self.doc_revision, count));
        count
    }
    fn folder_list(&mut self) -> Vec<(Uuid, String)> {
        if let Some((revision, folders)) = &self.folders_cache {
            if *revision == self.doc_revision {
                return folders.clone();
            }
        }
        let folders = self.doc.folders();
        self.folders_cache = Some((self.doc_revision, folders.clone()));
        folders
    }
    fn details(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.selected.iter().min().copied() else {
            ui.label("Select a row in the grid to inspect its details.");
            return;
        };
        let Some(mut bookmark) = self.doc.items().into_iter().find(|b| b.id == id) else {
            return;
        };
        let before = bookmark.clone();
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Bookmark details");
            ui.label("Title");
            ui.text_edit_singleline(&mut bookmark.title)
                .on_hover_text("Bookmark title");
            ui.label("URL");
            ui.text_edit_singleline(&mut bookmark.url)
                .on_hover_text("Bookmark URL. Stored exactly as typed.");
            if url::Url::parse(&bookmark.url).is_err() {
                ui.colored_label(theme::colors().danger, "This URL is not valid.");
            }
            ui.label("Notes");
            ui.add(
                egui::TextEdit::multiline(&mut bookmark.notes)
                    .desired_rows(6)
                    .desired_width(f32::INFINITY),
            )
            .on_hover_text("Free-form notes saved with the bookmark");
            ui.label("Tags (comma-separated)");
            let mut tags = bookmark.tags.join(", ");
            if ui
                .text_edit_singleline(&mut tags)
                .on_hover_text("Comma-separated tags, e.g. dev, rust")
                .changed()
            {
                bookmark.tags = tags
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect();
            }
            ui.separator();
            ui.label("Favicon (embedded locally; never fetched)");
            ui.horizontal(|ui| {
                if ui.button("Choose image…").on_hover_text("Import a PNG, JPEG, ICO or GIF as this bookmark's 32px favicon. Stored locally; nothing is fetched online.").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Images", &["png", "ico", "jpg", "jpeg", "gif"])
                        .pick_file()
                    {
                        let result = (|| -> parser::Result<String> {
                            if std::fs::metadata(&path)?.len() > 2 * 1024 * 1024 {
                                return Err("Favicon must be smaller than 2 MiB".into());
                            }
                            rustmarks::favicon::from_bytes(&std::fs::read(path)?)
                        })();
                        match result {
                            Ok(uri) => bookmark.favicon_base64 = Some(uri),
                            Err(e) => self.message = format!("Favicon import failed: {e}"),
                        }
                    }
                }
                if ui.button("Clear favicon").on_hover_text("Remove this bookmark's favicon.").clicked() {
                    bookmark.favicon_base64 = None;
                }
            });
            let mut icon = bookmark.favicon_base64.clone().unwrap_or_default();
            if ui
                .add(egui::TextEdit::singleline(&mut icon).hint_text("data:image/png;base64,…"))
                .on_hover_text("Favicon as a data: URL. Prefer Choose image… for local files.")
                .changed()
            {
                bookmark.favicon_base64 = if icon.is_empty() { None } else { Some(icon) };
            }
            if let Some(uri) = &bookmark.favicon_base64 {
                if self.icon_texture.as_ref().map(|(key, _)| key) != Some(uri) {
                    self.icon_texture = None;
                    match rustmarks::favicon::pixels(uri) {
                        Ok((w, h, pixels)) => {
                            let texture = ui.ctx().load_texture(
                                "bookmark-favicon",
                                egui::ColorImage::from_rgba_unmultiplied([w, h], &pixels),
                                egui::TextureOptions::LINEAR,
                            );
                            self.icon_texture = Some((uri.clone(), texture));
                        }
                        Err(e) => {
                            ui.colored_label(theme::colors().danger, format!("Cannot preview favicon: {e}"));
                        }
                    }
                }
                if let Some((_, texture)) = &self.icon_texture {
                    ui.image((texture.id(), egui::vec2(32., 32.)));
                }
            }
            ui.label(format!(
                "Added: {} · ID: {}",
                bookmark.date_added, bookmark.id
            ));
        });
        if bookmark != before {
            if let Some(BookmarkNode::Bookmark(target)) = self.doc.root_folder.find_mut(id) {
                *target = bookmark;
            }
        }
    }
    fn diff(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Staged changes");
            ui.checkbox(&mut self.unified, "Unified view")
                .on_hover_text("Show one combined diff instead of side by side");
            if components::primary_compact(ui, "Commit / Save", true).on_hover_text("Write the staged changes to disk. A safety snapshot is taken first.").clicked() {
                self.save(false);
            }
            if ui.button("Revert all…").on_hover_text("Discard all changes and restore the last committed document. A snapshot is taken first.").clicked() {
                self.pending_open = false;
                self.confirm_discard = true;
            }
        });
        let changes = diff_view::changes(&self.original, &self.doc);
        ui.label(format!(
            "{} changes · Original → Working copy",
            changes.len()
        ));
        if changes.is_empty() {
            ui.label("No changes to review.");
        }
        let kinds: std::collections::HashMap<_, _> =
            changes.iter().map(|c| (c.id, c.kind)).collect();
        fn tree(
            ui: &mut egui::Ui,
            node: &BookmarkNode,
            kinds: &std::collections::HashMap<Uuid, diff_view::Kind>,
            collapsed: &mut HashSet<Uuid>,
        ) {
            let color = match kinds.get(&node.id()) {
                Some(diff_view::Kind::Added) => theme::colors().success,
                Some(diff_view::Kind::Removed) => theme::colors().danger,
                Some(_) => theme::colors().warning,
                None => theme::colors().muted,
            };
            match node {
                BookmarkNode::Folder {
                    id,
                    title,
                    children,
                    ..
                } => {
                    ui.horizontal(|ui| {
                        if ui
                            .small_button(if collapsed.contains(id) { "+" } else { "−" })
                            .on_hover_text("Expand or collapse this folder in the diff")
                            .clicked()
                            && !collapsed.remove(id)
                        {
                            collapsed.insert(*id);
                        }
                        ui.colored_label(color, format!("{title}  ({})", children.len()));
                    });
                    if !collapsed.contains(id) {
                        ui.indent(id, |ui| {
                            for child in children {
                                tree(ui, child, kinds, collapsed);
                            }
                        });
                    }
                }
                BookmarkNode::Bookmark(b) => {
                    ui.colored_label(color, format!("• {}", b.title))
                        .on_hover_text(format!(
                            "{}\nTags: {}\n{}",
                            b.url,
                            b.tags.join(", "),
                            b.notes
                        ));
                }
            }
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            if !self.unified {
                ui.columns(2, |cols| {
                    cols[0].heading("Original");
                    cols[1].heading("Working copy");
                    cols[0].push_id("before-tree", |ui| {
                        tree(
                            ui,
                            &self.original.root_folder,
                            &kinds,
                            &mut self.diff_collapsed,
                        )
                    });
                    cols[1].push_id("after-tree", |ui| {
                        tree(ui, &self.doc.root_folder, &kinds, &mut self.diff_collapsed)
                    });
                });
                ui.separator();
                ui.label("Metadata changes — expand to inspect exact before / after values:");
            }
            for change in changes {
                let color = match change.kind {
                    diff_view::Kind::Added => theme::colors().success,
                    diff_view::Kind::Removed => theme::colors().danger,
                    _ => theme::colors().warning,
                };
                if self.unified {
                    ui.separator();
                    ui.colored_label(color, format!("{:?}  {}", change.kind, change.title));
                    if !change.before.is_empty() {
                        ui.colored_label(theme::colors().danger, format!("− {}", change.before));
                    }
                    if !change.after.is_empty() {
                        ui.colored_label(theme::colors().success, format!("+ {}", change.after));
                    }
                } else {
                    egui::CollapsingHeader::new(
                        RichText::new(format!("{:?}  {}", change.kind, change.title)).color(color),
                    )
                    .id_source(("change", change.id))
                    .show(ui, |ui| {
                        ui.columns(2, |cols| {
                            cols[0]
                                .label(RichText::new(change.before).color(theme::colors().danger));
                            cols[1]
                                .label(RichText::new(change.after).color(theme::colors().success));
                        });
                    });
                }
            }
        });
    }
    fn rescan_health(&mut self) {
        self.health = Some(rustmarks::health::scan(&self.doc));
    }
    /// Poll the watch folder (every ~2.5s while one is set). The first
    /// scan after launch or a folder change is a silent baseline; later
    /// scans flag new or changed bookmark files — never the file that is
    /// currently open.
    fn poll_watch(&mut self) {
        let Some(dir) = self.ui_settings.watch_dir.clone() else {
            return;
        };
        let due = self
            .watch_last_check
            .is_none_or(|t| t.elapsed() >= std::time::Duration::from_millis(2500));
        if !due {
            return;
        }
        self.watch_last_check = Some(std::time::Instant::now());
        let current = watch_dir_files(Path::new(&dir));
        if !self.watch_baseline {
            self.watch_seen = current;
            self.watch_baseline = true;
            return;
        }
        let open_path = self.doc.source_path.as_ref().map(PathBuf::from);
        let mut newest: Option<(PathBuf, std::time::SystemTime)> = None;
        for (path, mtime) in &current {
            if self.watch_seen.get(path) != Some(mtime)
                && open_path.as_ref() != Some(path)
                && newest.as_ref().is_none_or(|(_, t)| *mtime > *t)
            {
                newest = Some((path.clone(), *mtime));
            }
        }
        if let Some((path, _)) = newest {
            if self.watch_found.as_ref() != Some(&path) {
                self.watch_found = Some(path);
            }
        }
        self.watch_seen = current;
    }
    fn health_view(&mut self, ui: &mut egui::Ui) {
        use rustmarks::health::IssueKind;
        ui.heading("Library Health");
        theme::active_rule(ui);
        ui.label(
            "A fully local scan for duplicates, empty folders, missing titles, tracking parameters and malformed URLs. Nothing is checked over the network, and nothing changes until you fix it.",
        );
        ui.add_space(8.);
        if self.health.is_none() {
            self.rescan_health();
        }
        let report = self.health.clone().unwrap_or_default();
        components::card(ui, |ui| {
            ui.columns(6, |columns| {
                components::metric(
                    &mut columns[0],
                    report.bookmarks,
                    "Bookmarks scanned",
                    theme::colors().progress,
                );
                components::metric(
                    &mut columns[1],
                    report.count(IssueKind::DuplicateUrl),
                    "Duplicate URLs",
                    theme::colors().info,
                );
                components::metric(
                    &mut columns[2],
                    report.count(IssueKind::EmptyFolder),
                    "Empty folders",
                    theme::colors().warning,
                );
                components::metric(
                    &mut columns[3],
                    report.count(IssueKind::MissingTitle),
                    "Missing titles",
                    theme::colors().warning,
                );
                components::metric(
                    &mut columns[4],
                    report.count(IssueKind::TrackingParameters),
                    "Tracking parameters",
                    theme::colors().info,
                );
                components::metric(
                    &mut columns[5],
                    report.count(IssueKind::MalformedUrl),
                    "Malformed URLs",
                    theme::colors().danger,
                );
            });
        });
        ui.add_space(10.);
        ui.horizontal(|ui| {
            if components::primary_compact(ui, "Rescan", true)
                .on_hover_text("Run the health scan again against the current library.")
                .clicked()
            {
                self.rescan_health();
            }
            if report.is_clean() {
                ui.label(
                    RichText::new("No issues found — this library is healthy.")
                        .color(theme::colors().success),
                );
            }
        });
        for kind in IssueKind::ALL {
            let issues: Vec<_> = report
                .issues
                .iter()
                .filter(|i| i.kind == kind)
                .cloned()
                .collect();
            if issues.is_empty() {
                continue;
            }
            components::section(ui, kind.label());
            if issues.iter().any(|i| i.fixable) {
                let fixable: Vec<_> = issues.iter().filter(|i| i.fixable).cloned().collect();
                if ui
                    .button(format!("Fix all {} in this group", fixable.len()))
                    .on_hover_text(
                        "Apply the automatic fix to every fixable issue in this group. A snapshot is taken first; undo with Ctrl+Z.",
                    )
                    .clicked()
                    && self.snapshot("health-fix")
                {
                    let mut fixed = 0;
                    for issue in &fixable {
                        if rustmarks::health::fix(&mut self.doc, issue) {
                            fixed += 1;
                        }
                    }
                    self.message = format!("Fixed {fixed} issue(s).");
                    self.rescan_health();
                    return;
                }
            }
            for issue in &issues {
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(&issue.detail).truncate())
                        .on_hover_text(&issue.detail);
                    if issue.fixable {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .small_button("Fix")
                                .on_hover_text(
                                    "Apply the automatic fix. A snapshot is taken first; undo with Ctrl+Z.",
                                )
                                .clicked()
                                && self.snapshot("health-fix")
                                && rustmarks::health::fix(&mut self.doc, issue)
                            {
                                self.message = "Fixed.".into();
                                self.rescan_health();
                            }
                        });
                    } else {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new("Fix in the Library grid")
                                    .small()
                                    .color(theme::colors().muted),
                            );
                        });
                    }
                });
            }
        }
    }
    fn organizer(&mut self, ui: &mut egui::Ui) {
        ui.heading("Smart Organizer");
        theme::active_rule(ui);
        ui.label("Local domain and keyword rules. No cloud services, no tracking.");
        ui.add_space(12.);
        ui.horizontal(|ui| {
            if components::primary_compact(ui, "Scan suggestions", true).on_hover_text("Scan bookmarks with the local rules and propose folders. Does not move anything.").clicked() && self.snapshot("suggest") {
                heuristics::suggest_with(&mut self.doc, &self.rules);
                self.message = "Suggestions ready. Review below before organizing.".into();
            }
            if ui.button("Accept all → folders").on_hover_text("Move every suggested bookmark into its category folder. A snapshot is taken first.").clicked() && self.snapshot("organize") {
                heuristics::organize(&mut self.doc);
                self.tab = Tab::Diff;
                self.view = View::Explorer;
            }
        });
        egui::ScrollArea::vertical().show(ui, |ui| {
            for b in self
                .doc
                .items()
                .iter()
                .filter(|b| b.category_guess.is_some())
            {
                ui.horizontal(|ui| {
                    ui.label(&b.title);
                    ui.colored_label(theme::colors().success, b.category_guess.as_deref().unwrap_or(""));
                    if ui.button("Accept").on_hover_text("Move this bookmark into the suggested folder. A snapshot is taken first.").clicked() && self.snapshot("categorize") {
                        // Move this bookmark only; retain its identity for the staged diff.
                        let category = b.category_guess.clone().unwrap();
                        let mut ids = HashSet::new();
                        ids.insert(b.id);
                        self.doc.root_folder.remove(&ids);
                        let children = self.doc.root_folder.children_mut().unwrap();
                        if let Some(BookmarkNode::Folder { children, .. }) =
                            children.iter_mut().find(
                                |n| matches!(n,BookmarkNode::Folder{title,..} if title==&category),
                            )
                        {
                            children.push(BookmarkNode::Bookmark(b.clone()));
                        } else {
                            let mut folder = BookmarkNode::folder(category);
                            folder
                                .children_mut()
                                .unwrap()
                                .push(BookmarkNode::Bookmark(b.clone()));
                            children.push(folder);
                        }
                        if let Some(BookmarkNode::Bookmark(item)) =
                            self.doc.root_folder.find_mut(b.id)
                        {
                            item.category_guess = None;
                        }
                    }
                });
                ui.separator();
            }
        });
    }
    fn merge_panel(&mut self, ui: &mut egui::Ui) {
        match self
            .merge_ui
            .show(ui, &self.doc, self.merge_session.as_mut())
        {
            MergeAction::None => {}
            MergeAction::OpenLibrary => self.request_open(),
            MergeAction::ChooseFiles => self.begin_merge(),
            MergeAction::Refresh => self.refresh_merge(),
            MergeAction::Clear => {
                self.merge_session = None;
                self.merge_ui.reset();
            }
            MergeAction::Review => self.stage_merge(),
        }
    }
    fn history_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Version History");
        theme::active_rule(ui);
        ui.label("Compressed local snapshots. Restoring stages a working copy; it does not overwrite your file.");
        ui.horizontal(|ui| {
            if ui
                .button("Refresh")
                .on_hover_text("Reload the list of saved snapshots from disk.")
                .clicked()
            {
                match self.history.list() {
                    Ok(s) => self.snapshots = s,
                    Err(e) => self.message = e.to_string(),
                }
            }
            if components::primary_compact(ui, "Create snapshot", true)
                .on_hover_text("Save a compressed snapshot of the working copy to local history.")
                .clicked()
                && self.snapshot("manual")
            {
                self.snapshots = self.history.list().unwrap_or_default();
            }
        });
        {
            let keep = self.ui_settings.snapshot_keep_last;
            let bytes: u64 = self
                .snapshots
                .iter()
                .filter_map(|s| std::fs::metadata(&s.path).ok())
                .map(|m| m.len())
                .sum();
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{} snapshot(s) · {:.1} MB on disk · {}",
                    self.snapshots.len(),
                    bytes as f64 / 1_048_576.0,
                    if keep == 0 {
                        "keeping all (set a limit in Settings)".to_string()
                    } else {
                        format!("keeping the newest {keep}")
                    }
                ));
                if keep > 0
                    && self.snapshots.len() > keep as usize
                    && ui
                        .button("Prune now")
                        .on_hover_text(
                            "Delete the oldest snapshots beyond the keep limit, right now.",
                        )
                        .clicked()
                {
                    match self.history.prune(keep as usize) {
                        Ok(removed) => {
                            self.message = format!("Removed {removed} old snapshot(s).");
                            self.snapshots = self.history.list().unwrap_or_default();
                        }
                        Err(e) => self.message = e.to_string(),
                    }
                }
            });
        }
        if self.snapshots.is_empty() {
            ui.label("No snapshots loaded. Refresh to list saved versions.");
        }
        let mut restore = None;
        let mut inspect = None;
        egui::ScrollArea::vertical()
            .max_height(300.)
            .show(ui, |ui| {
                for s in &self.snapshots {
                    ui.horizontal(|ui| {
                        if ui.button("Inspect").on_hover_text("Compare this snapshot with the working copy without changing anything.").clicked() {
                            inspect = Some(s.clone());
                        }
                        if ui.button("Restore").on_hover_text("Load this snapshot into the working copy. Review and save to commit; a snapshot is taken first.").clicked() {
                            restore = Some(s.clone());
                        }
                        ui.label(&s.label);
                    });
                }
            });
        if let Some(snapshot) = inspect {
            match self.history.restore(&snapshot) {
                Ok(doc) => self.inspected_snapshot = Some((snapshot, doc)),
                Err(e) => self.message = format!("Snapshot inspection failed: {e}"),
            }
        }
        if let Some((snapshot, doc)) = &self.inspected_snapshot {
            ui.separator();
            ui.label(format!(
                "Inspecting {} · {} bookmarks",
                snapshot.label,
                doc.items().len()
            ));
            ui.label("Differences from this snapshot to your current working copy:");
            egui::ScrollArea::vertical()
                .id_source("history-preview")
                .max_height(250.)
                .show(ui, |ui| {
                    for change in diff_view::changes(doc, &self.doc) {
                        egui::CollapsingHeader::new(format!("{:?}: {}", change.kind, change.title))
                            .id_source(("history-change", change.id))
                            .show(ui, |ui| {
                                ui.columns(2, |columns| {
                                    columns[0].label(change.before);
                                    columns[1].label(change.after);
                                });
                            });
                    }
                });
        }
        if let Some(s) = restore {
            if self.snapshot("before-restore") {
                match self.history.restore(&s) {
                    Ok(mut doc) => {
                        doc.source_path = self.doc.source_path.clone();
                        self.doc = doc;
                        self.folder = None;
                        self.selected.clear();
                        self.view = View::Explorer;
                        self.tab = Tab::Diff;
                        self.message =
                            "Snapshot restored to working copy. Review and save to commit.".into();
                    }
                    Err(e) => self.message = e.to_string(),
                }
            }
        }
    }
    fn settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settings & help");
        theme::active_rule(ui);
        ui.label(format!(
            "RustMarks {} · Tessel design · Offline-first",
            env!("CARGO_PKG_VERSION")
        ));
        ui.separator();
        components::section(ui, "Appearance");
        ui.label(
            RichText::new("Tessel design system. Theme follows the OS unless you fix it here; the accent preset recolors the action, selection and focus together.")
                .small()
                .color(theme::colors().muted),
        );
        let before = self.ui_settings.clone();
        ui.horizontal(|ui| {
            ui.label("Theme");
            ui.selectable_value(&mut self.ui_settings.theme, ThemeMode::System, "System")
                .on_hover_text("Follow the operating system's light/dark setting");
            ui.selectable_value(&mut self.ui_settings.theme, ThemeMode::Dark, "Dark")
                .on_hover_text("Always use the Tessel dark theme");
            ui.selectable_value(&mut self.ui_settings.theme, ThemeMode::Light, "Light")
                .on_hover_text("Always use the Tessel light theme");
        });
        ui.horizontal(|ui| {
            ui.label("Accent");
            ui.selectable_value(
                &mut self.ui_settings.accent,
                AccentPreset::Crimson,
                "Crimson",
            )
            .on_hover_text("Tessel's brand crimson");
            ui.selectable_value(&mut self.ui_settings.accent, AccentPreset::Blue, "Blue")
                .on_hover_text("Blue preset");
            ui.selectable_value(&mut self.ui_settings.accent, AccentPreset::Teal, "Teal")
                .on_hover_text("Teal preset");
            ui.selectable_value(
                &mut self.ui_settings.accent,
                AccentPreset::Graphite,
                "Graphite",
            )
            .on_hover_text("Neutral graphite, for color-critical work");
        });
        ui.horizontal(|ui| {
            ui.label("Density");
            ui.selectable_value(
                &mut self.ui_settings.density,
                Density::PerComponent,
                "Per component",
            )
            .on_hover_text("Compact rows and toolbars; comfortable navigation, forms and dialogs");
            ui.selectable_value(&mut self.ui_settings.density, Density::Compact, "Compact")
                .on_hover_text("Compact density everywhere");
            ui.selectable_value(
                &mut self.ui_settings.density,
                Density::Comfortable,
                "Comfortable",
            )
            .on_hover_text("Comfortable density everywhere");
            ui.selectable_value(&mut self.ui_settings.density, Density::Touch, "Touch")
                .on_hover_text("Large touch targets everywhere");
        });
        if self.ui_settings != before {
            let ctx = ui.ctx().clone();
            self.appearance_changed(&ctx);
        }
        components::section(ui, "Snapshots");
        ui.label(
            "History snapshots are pruned automatically after each new one: only the newest ones are kept.",
        );
        ui.horizontal(|ui| {
            ui.label("Keep");
            for (value, label) in [
                (25_u32, "25"),
                (50, "50"),
                (100, "100"),
                (250, "250"),
                (0, "All"),
            ] {
                if ui
                    .selectable_value(&mut self.ui_settings.snapshot_keep_last, value, label)
                    .on_hover_text("How many snapshots to keep; older ones are deleted")
                    .clicked()
                {
                    if let Err(e) = self.ui_settings.save() {
                        self.message = e.to_string();
                    }
                }
            }
            ui.label("snapshots");
        });
        ui.separator();
        components::section(ui, "Watch folder");
        ui.label(
            "Pick a folder your browser exports bookmarks to. When a new or changed export file appears there, RustMarks offers to merge it — nothing is imported without your click.",
        );
        ui.horizontal(|ui| {
            ui.label(
                self.ui_settings
                    .watch_dir
                    .as_deref()
                    .unwrap_or("Not watching any folder"),
            );
            if ui
                .button("Choose…")
                .on_hover_text("Choose the folder to watch for new bookmark exports.")
                .clicked()
            {
                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    self.ui_settings.watch_dir = Some(dir.display().to_string());
                    self.watch_baseline = false;
                    self.watch_found = None;
                    if let Err(e) = self.ui_settings.save() {
                        self.message = e.to_string();
                    }
                }
            }
            if self.ui_settings.watch_dir.is_some()
                && ui
                    .button("Stop watching")
                    .on_hover_text("Stop watching the folder. Nothing is deleted.")
                    .clicked()
            {
                self.ui_settings.watch_dir = None;
                self.watch_found = None;
                if let Err(e) = self.ui_settings.save() {
                    self.message = e.to_string();
                }
            }
        });
        ui.separator();
        ui.label("Ctrl+K  Command palette    Ctrl+,  Settings    F1  Help");
        ui.label("Ctrl+O  Open    Ctrl+S  Save    Ctrl+Shift+S  Save as");
        ui.label("Ctrl-click rows to toggle selection. Shift-click to select a range.");
        ui.label("Search supports plain terms, tag:dev, and negative terms such as -broken.");
        ui.label("JSON saves preserve the complete RustMarks model. HTML preserves folders, titles, URLs, dates, tags, notes and icon data.");
        ui.label("Chrome JSON export preserves browser-supported fields only; notes, tags and icon data are not included.");
        ui.label(format!(
            "Snapshot directory: {}",
            self.history.directory.display()
        ));
        ui.separator();
        if ui.button("Export Chrome / Edge JSON…").on_hover_text("Export a browser-compatible JSON file. RustMarks-only metadata (notes, tags) is omitted.").clicked() {
            self.export_chrome_json();
        }
        ui.separator();
        ui.heading("Local categorization dictionary");
        ui.label("Earlier rules win equal scores. Domains match exact hosts and subdomains; keywords match whole tokens.");
        ui.add(
            egui::TextEdit::multiline(&mut self.rules_text)
                .code_editor()
                .desired_rows(14)
                .desired_width(f32::INFINITY),
        )
        .on_hover_text(
            "Categorization rules as JSON. Validate and save to apply; bookmarks are not moved.",
        );
        ui.horizontal(|ui| {
            if components::primary_compact(ui, "Validate and save rules", true).on_hover_text("Check the rules JSON and, if valid, save it as the active dictionary. Bookmarks are not moved.").clicked() {
                match RuleSet::parse(&self.rules_text) {
                    Ok(rules) => {
                        let path = self
                            .history
                            .directory
                            .parent()
                            .unwrap_or(&self.history.directory)
                            .join("rules.json");
                        let result = (|| -> parser::Result<()> {
                            std::fs::create_dir_all(path.parent().unwrap())?;
                            rustmarks::storage::atomic_write(&path, self.rules_text.as_bytes())
                        })();
                        match result {
                            Ok(()) => {
                                self.rules = rules;
                                self.message =
                                    "Rules saved locally. Scan suggestions to apply them.".into();
                            }
                            Err(e) => self.message = format!("Rules save failed: {e}"),
                        }
                    }
                    Err(e) => {
                        self.message = format!("Invalid rules; active dictionary unchanged: {e}")
                    }
                }
            }
            if ui.button("Load default rules into editor").on_hover_text("Replace the editor text with the built-in rules. Nothing is saved until you validate and save.").clicked() {
                self.rules_text =
                    serde_json::to_string_pretty(&RuleSet::default()).unwrap_or_default();
            }
        });
        ui.label("Ctrl+Z / Ctrl+Y undo or redo document edits when no text field is focused. Text fields keep their own undo while editing.");
        ui.label("For lossless editing, use RustMarks JSON as your working file and export HTML for your browser.");
    }
}
/// Every action the command palette (Ctrl+K) can run. The palette
/// complements the visible controls; it never replaces them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Command {
    Open,
    Save,
    SaveAs,
    MergeFiles,
    AddBookmark,
    ScanSuggestions,
    CleanTrackingUrls,
    CreateSnapshot,
    ExportChromeJson,
    Undo,
    Redo,
    GoLibrary,
    GoMerge,
    GoCompare,
    GoOrganize,
    GoHistory,
    GoHealth,
    GoSettings,
}

struct CommandInfo {
    command: Command,
    label: &'static str,
    keywords: &'static str,
    shortcut: Option<&'static str>,
    disabled: Option<&'static str>,
}

/// Subsequence fuzzy match: every query character must appear in order.
/// Word starts and contiguous runs score higher. `None` means no match.
/// Bookmark files (`.html`, `.htm`, `.json`) in a folder, with their
/// modification times — the watch folder's raw scan.
fn watch_dir_files(dir: &Path) -> std::collections::HashMap<PathBuf, std::time::SystemTime> {
    let mut out = std::collections::HashMap::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("html" | "htm" | "json")
            ) {
                if let Ok(mtime) = entry.metadata().and_then(|m| m.modified()) {
                    out.insert(path, mtime);
                }
            }
        }
    }
    out
}

fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let query: Vec<char> = query.to_lowercase().chars().collect();
    let text: Vec<char> = text.to_lowercase().chars().collect();
    let mut score = 0_i32;
    let mut qi = 0;
    let mut previous_matched = false;
    for (index, ch) in text.iter().enumerate() {
        if qi < query.len() && *ch == query[qi] {
            score += 1;
            if index == 0 || !text[index - 1].is_alphanumeric() {
                score += 4;
            }
            if previous_matched {
                score += 6;
            }
            previous_matched = true;
            qi += 1;
        } else {
            previous_matched = false;
        }
    }
    (qi == query.len()).then_some(score)
}

impl Studio {
    fn command_catalog(&self) -> Vec<CommandInfo> {
        let entry = |command, label, keywords, shortcut, disabled| CommandInfo {
            command,
            label,
            keywords,
            shortcut,
            disabled,
        };
        vec![
            entry(
                Command::Open,
                "Open bookmarks…",
                "file load import html json",
                Some("Ctrl+O"),
                None,
            ),
            entry(
                Command::Save,
                "Save",
                "write commit disk",
                Some("Ctrl+S"),
                None,
            ),
            entry(
                Command::SaveAs,
                "Save as…",
                "write new file export",
                Some("Ctrl+Shift+S"),
                None,
            ),
            entry(
                Command::MergeFiles,
                "Merge files…",
                "combine import duplicates",
                None,
                None,
            ),
            entry(
                Command::AddBookmark,
                "Add bookmark",
                "new create entry",
                None,
                None,
            ),
            entry(
                Command::ScanSuggestions,
                "Scan categorization suggestions",
                "organize rules propose folders",
                None,
                None,
            ),
            entry(
                Command::CleanTrackingUrls,
                "Clean tracking URLs of selected",
                "utm fbclid gclid remove parameters",
                None,
                self.selected.is_empty().then_some("Select bookmarks first"),
            ),
            entry(
                Command::CreateSnapshot,
                "Create snapshot",
                "history backup version",
                None,
                None,
            ),
            entry(
                Command::ExportChromeJson,
                "Export Chrome / Edge JSON…",
                "browser export",
                None,
                None,
            ),
            entry(
                Command::Undo,
                "Undo document edit",
                "revert",
                Some("Ctrl+Z"),
                (!self.undo.can_undo()).then_some("Nothing to undo"),
            ),
            entry(
                Command::Redo,
                "Redo document edit",
                "reapply",
                Some("Ctrl+Y"),
                (!self.undo.can_redo()).then_some("Nothing to redo"),
            ),
            entry(
                Command::GoLibrary,
                "Go to Library",
                "view grid bookmarks",
                Some("Ctrl+1"),
                None,
            ),
            entry(
                Command::GoMerge,
                "Go to Merge",
                "view combine",
                Some("Ctrl+2"),
                None,
            ),
            entry(
                Command::GoCompare,
                "Go to Compare",
                "view changes diff staged",
                Some("Ctrl+3"),
                None,
            ),
            entry(
                Command::GoOrganize,
                "Go to Organize",
                "view suggestions folders",
                Some("Ctrl+4"),
                None,
            ),
            entry(
                Command::GoHealth,
                "Go to Library Health",
                "view scan duplicates empty folders tracking malformed",
                Some("Ctrl+5"),
                None,
            ),
            entry(
                Command::GoHistory,
                "Go to History",
                "view snapshots versions",
                Some("Ctrl+6"),
                None,
            ),
            entry(
                Command::GoSettings,
                "Go to Settings & help",
                "view appearance rules shortcuts",
                Some("Ctrl+7"),
                None,
            ),
        ]
    }

    fn palette_matches(&self) -> Vec<CommandInfo> {
        let query = self.palette_query.trim().to_lowercase();
        let mut scored: Vec<(i32, usize, CommandInfo)> = self
            .command_catalog()
            .into_iter()
            .enumerate()
            .filter_map(|(index, info)| {
                let label_score = fuzzy_score(&query, info.label);
                let keyword_score = fuzzy_score(&query, info.keywords).map(|s| s - 2);
                label_score
                    .max(keyword_score)
                    .map(|score| (score, index, info))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        scored.into_iter().map(|(_, _, info)| info).collect()
    }

    fn run_command(&mut self, command: Command) {
        match command {
            Command::Open => self.request_open(),
            Command::Save => self.save(false),
            Command::SaveAs => self.save(true),
            Command::MergeFiles => self.begin_merge(),
            Command::AddBookmark => {
                self.navigate(View::Explorer, Tab::Grid);
                self.add_bookmark();
            }
            Command::ScanSuggestions => {
                if self.snapshot("suggest") {
                    heuristics::suggest_with(&mut self.doc, &self.rules);
                    self.view = View::Organizer;
                    self.message = "Suggestions ready. Review before organizing.".into();
                }
            }
            Command::CleanTrackingUrls => {
                if !self.selected.is_empty() && self.snapshot("clean-urls") {
                    let selected = self.selected.clone();
                    self.doc.root_folder.visit_mut(&mut |b| {
                        if selected.contains(&b.id) {
                            b.url = rustmarks::url_cleanup::clean(&b.url);
                        }
                    });
                }
            }
            Command::CreateSnapshot => {
                if self.snapshot("manual") {
                    self.snapshots = self.history.list().unwrap_or_default();
                    self.message = "Snapshot saved to local history.".into();
                }
            }
            Command::ExportChromeJson => self.export_chrome_json(),
            Command::Undo => self.undo_edit(),
            Command::Redo => self.redo_edit(),
            Command::GoLibrary => self.navigate(View::Explorer, Tab::Grid),
            Command::GoMerge => self.navigate(View::Merge, Tab::Grid),
            Command::GoCompare => self.navigate(View::Explorer, Tab::Diff),
            Command::GoOrganize => self.navigate(View::Organizer, Tab::Grid),
            Command::GoHealth => self.navigate(View::Health, Tab::Grid),
            Command::GoHistory => self.navigate(View::History, Tab::Grid),
            Command::GoSettings => self.navigate(View::Settings, Tab::Grid),
        }
    }

    /// Ctrl+K overlay: fuzzy search over every command, arrow keys move,
    /// Enter runs, Esc closes. Disabled commands explain why.
    fn command_palette(&mut self, ctx: &egui::Context) {
        let matches = self.palette_matches();
        if self.palette_index >= matches.len() {
            self.palette_index = matches.len().saturating_sub(1);
        }
        let mut run: Option<Command> = None;
        let mut close = false;
        ctx.input_mut(|input| {
            if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                self.palette_index = (self.palette_index + 1).min(matches.len().saturating_sub(1));
            }
            if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                self.palette_index = self.palette_index.saturating_sub(1);
            }
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Enter) {
                if let Some(info) = matches.get(self.palette_index) {
                    if info.disabled.is_none() {
                        run = Some(info.command);
                    }
                }
            }
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                close = true;
            }
        });
        egui::Window::new("Command palette")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size(egui::vec2(540., 0.))
            .anchor(egui::Align2::CENTER_TOP, [0., 90.])
            .show(ctx, |ui| {
                let search = ui.add(
                    egui::TextEdit::singleline(&mut self.palette_query)
                        .hint_text("Type a command…")
                        .desired_width(f32::INFINITY),
                );
                if self.palette_focus {
                    search.request_focus();
                    self.palette_focus = false;
                }
                if search.changed() {
                    self.palette_index = 0;
                }
                ui.separator();
                if matches.is_empty() {
                    ui.label(RichText::new("No commands match.").color(theme::colors().muted));
                }
                egui::ScrollArea::vertical()
                    .max_height(8. * 30.)
                    .show(ui, |ui| {
                        for (index, info) in matches.iter().enumerate() {
                            let selected = index == self.palette_index;
                            ui.horizontal(|ui| {
                                if let Some(reason) = info.disabled {
                                    ui.add_enabled(
                                        false,
                                        egui::SelectableLabel::new(false, info.label),
                                    )
                                    .on_disabled_hover_text(reason);
                                    ui.label(
                                        RichText::new(reason).small().color(theme::colors().muted),
                                    );
                                } else if ui
                                    .selectable_label(selected, info.label)
                                    .on_hover_text("Run this command")
                                    .clicked()
                                {
                                    run = Some(info.command);
                                }
                                if let Some(shortcut) = info.shortcut {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                RichText::new(shortcut)
                                                    .small()
                                                    .color(theme::colors().muted),
                                            );
                                        },
                                    );
                                }
                            });
                        }
                    });
            });
        if let Some(command) = run {
            self.run_command(command);
            close = true;
        }
        if close {
            self.palette_open = false;
            self.palette_query.clear();
            self.palette_index = 0;
        }
    }
}

impl eframe::App for Studio {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.navigating_history = false;
        self.persist_window_state(ctx);
        if self.ui_settings.theme == ThemeMode::System {
            // Follow live OS light/dark changes while running.
            theme::apply(ctx, &self.ui_settings);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::K)) {
            self.palette_open = !self.palette_open;
            if self.palette_open {
                self.palette_query.clear();
                self.palette_index = 0;
                self.palette_focus = true;
            }
        }
        if !self.palette_open {
            if !ctx.wants_keyboard_input() {
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Z)) {
                    self.undo_edit();
                }
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Y)) {
                    self.redo_edit();
                }
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::O)) {
                if self.dirty() {
                    self.pending_open = true;
                    self.confirm_discard = true;
                } else {
                    self.open();
                }
            }
            if ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::S)
            }) {
                self.save(true);
            } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::S)) {
                self.save(false);
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Comma)) {
                self.navigate(View::Settings, Tab::Grid);
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F1)) {
                self.navigate(View::Settings, Tab::Grid);
            }
            for (key, view, tab) in [
                (egui::Key::Num1, View::Explorer, Tab::Grid),
                (egui::Key::Num2, View::Merge, Tab::Grid),
                (egui::Key::Num3, View::Explorer, Tab::Diff),
                (egui::Key::Num4, View::Organizer, Tab::Grid),
                (egui::Key::Num5, View::Health, Tab::Grid),
                (egui::Key::Num6, View::History, Tab::Grid),
                (egui::Key::Num7, View::Settings, Tab::Grid),
            ] {
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, key)) {
                    self.navigate(view, tab);
                }
            }
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.dirty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm_close = true;
        }
        self.poll_watch();
        self.workspace(ctx);
        if self.confirm_discard {
            egui::Window::new("Discard staged changes?")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0., 0.])
                .show(ctx, |ui| {
                    ui.label(
                        "Your unsaved edits will be discarded. A safety snapshot is created first.",
                    );
                    ui.horizontal(|ui| {
                        if components::danger(ui, "Discard changes", true)
                            .on_hover_text(
                                "Throw away unsaved changes. A safety snapshot is taken first.",
                            )
                            .clicked()
                            && self.snapshot("before-discard")
                        {
                            self.doc = self.original.clone();
                            self.confirm_discard = false;
                            if self.pending_open {
                                self.pending_open = false;
                                self.open();
                            }
                            if let Some(path) = self.pending_drop.take() {
                                self.open_path(path);
                            }
                        }
                        if ui
                            .button("Keep editing")
                            .on_hover_text("Close this prompt and return to the editor.")
                            .clicked()
                        {
                            self.confirm_discard = false;
                            self.pending_open = false;
                            self.pending_drop = None;
                        }
                    });
                });
        }
        if self.confirm_close {
            egui::Window::new("Unsaved changes")
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, [0., 0.])
                .show(ctx, |ui| {
                    ui.label("Save your changes before closing RustMarks?");
                    ui.horizontal(|ui| {
                        if components::primary_compact(ui, "Save", true)
                            .on_hover_text("Save the document, then continue.")
                            .clicked()
                        {
                            self.save(false);
                            if !self.dirty() {
                                self.confirm_close = false;
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                        if ui
                            .button("Discard and close")
                            .on_hover_text(
                                "Close without saving. A safety snapshot is taken first.",
                            )
                            .clicked()
                            && self.snapshot("before-close")
                        {
                            self.original = self.doc.clone();
                            self.confirm_close = false;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui
                            .button("Cancel")
                            .on_hover_text("Close this prompt; nothing changes.")
                            .clicked()
                        {
                            self.confirm_close = false;
                        }
                    });
                });
        }
        // Drag-and-drop: dropping a bookmark file on the window opens it,
        // with the same unsaved-changes protection as Ctrl+O.
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if let Some(path) = dropped.into_iter().find(|p| {
            matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("html" | "htm" | "json")
            )
        }) {
            if self.dirty() {
                self.pending_drop = Some(path);
                self.confirm_discard = true;
            } else {
                self.open_path(path);
            }
        } else if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            egui::Area::new(egui::Id::new("drop_hint"))
                .anchor(egui::Align2::CENTER_CENTER, [0., 0.])
                .show(ctx, |ui| {
                    components::card(ui, |ui| {
                        ui.label(
                            RichText::new("Drop to open the bookmark file")
                                .size(16.)
                                .strong(),
                        );
                    });
                });
        }
        if self.palette_open {
            self.command_palette(ctx);
        }
        if self.doc != self.checkpoint {
            self.doc_revision += 1;
            if !self.navigating_history {
                let now = std::time::Instant::now();
                let coalesce = ctx.wants_keyboard_input()
                    && self
                        .last_edit
                        .is_some_and(|t| now.duration_since(t).as_millis() < 500);
                self.undo.record(
                    std::mem::replace(&mut self.checkpoint, self.doc.clone()),
                    coalesce,
                );
                self.last_edit = if ctx.wants_keyboard_input() {
                    Some(now)
                } else {
                    None
                };
            } else {
                self.checkpoint = self.doc.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn render_tabs(
        context: &egui::Context,
        studio: &mut Studio,
        events: Vec<egui::Event>,
    ) -> [egui::Rect; 3] {
        let mut rects = [egui::Rect::NOTHING; 3];
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000., 650.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    rects = studio.editor_tabs(ui).map(|response| response.rect);
                });
            },
        );
        rects
    }
    #[test]
    fn changes_tab_marker_tracks_edits_undo_redo_and_save_boundary() {
        let mut studio = Studio::default();
        assert_eq!(studio.changes_tab_label(), "⇄  Changes");
        studio.undo.record(studio.doc.clone(), false);
        studio.doc.add(
            None,
            BookmarkNode::Bookmark(BookmarkItem::new("Example", "https://example.com")),
        );
        assert_eq!(studio.changes_tab_label(), "⇄  Changes ●");
        studio.undo_edit();
        assert_eq!(studio.changes_tab_label(), "⇄  Changes");
        studio.redo_edit();
        assert_eq!(studio.changes_tab_label(), "⇄  Changes ●");
        // The successful-save boundary updates original without changing the document.
        studio.original = studio.doc.clone();
        assert_eq!(studio.changes_tab_label(), "⇄  Changes");
        studio
            .doc
            .root_folder
            .visit_mut(&mut |bookmark| bookmark.notes = "Changed".into());
        assert_eq!(studio.changes_tab_label(), "⇄  Changes ●");
        studio.doc = studio.original.clone();
        assert_eq!(studio.changes_tab_label(), "⇄  Changes");
    }
    #[test]
    fn clean_and_modified_tab_bars_render_and_accept_pointer_selection() {
        for modified in [false, true] {
            let context = egui::Context::default();
            crate::tessel::apply(&context, &rustmarks::ui_settings::UiSettings::default());
            let mut studio = Studio::default();
            if modified {
                studio.doc.add(
                    None,
                    BookmarkNode::Bookmark(BookmarkItem::new("Example", "https://example.com")),
                );
            }
            let mut rects = render_tabs(&context, &mut studio, vec![]);
            for (index, expected) in [(1, Tab::Details), (2, Tab::Diff), (0, Tab::Grid)] {
                let position = rects[index].center();
                assert!(position.is_finite());
                assert!(rects[index].width() > 0.);
                render_tabs(
                    &context,
                    &mut studio,
                    vec![
                        egui::Event::PointerMoved(position),
                        egui::Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
                rects = render_tabs(
                    &context,
                    &mut studio,
                    vec![egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                );
                assert_eq!(studio.tab, expected);
                assert_eq!(studio.dirty(), modified);
            }
        }
    }
    #[test]
    fn fuzzy_matching_and_palette_filtering() {
        assert!(fuzzy_score("sv", "Save").is_some());
        assert!(fuzzy_score("xyz", "Save").is_none());
        assert!(fuzzy_score("", "anything").is_some());
        // Word-start matches outrank scattered ones.
        assert!(
            fuzzy_score("sa", "Save as…").unwrap()
                > fuzzy_score("sa", "Organize scans").unwrap_or(0)
        );
        let mut studio = Studio {
            palette_query: "snap".into(),
            ..Studio::default()
        };
        let matches = studio.palette_matches();
        assert_eq!(
            matches.first().map(|m| m.command),
            Some(Command::CreateSnapshot)
        );
        studio.palette_query = "settings".into();
        let matches = studio.palette_matches();
        assert!(matches.iter().any(|m| m.command == Command::GoSettings));
        // Disabled commands carry their reason into the palette.
        studio.palette_query = "clean".into();
        let matches = studio.palette_matches();
        let clean = matches
            .iter()
            .find(|m| m.command == Command::CleanTrackingUrls)
            .unwrap();
        assert_eq!(clean.disabled, Some("Select bookmarks first"));
    }
    struct TempHistory(PathBuf);
    impl TempHistory {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("rustmarks-shell-test-{}", Uuid::new_v4())))
        }
    }
    impl Drop for TempHistory {
        fn drop(&mut self) {
            if self.0.is_dir() {
                let _ = std::fs::remove_dir_all(&self.0);
            } else {
                let _ = std::fs::remove_file(&self.0);
            }
        }
    }
    fn merge_fixture() -> Studio {
        let mut studio = Studio::default();
        studio.doc.add(
            None,
            BookmarkNode::Bookmark(BookmarkItem::new("Existing", "https://example.com")),
        );
        studio.original = studio.doc.clone();
        studio.checkpoint = studio.doc.clone();
        let mut incoming = BookmarkDocument::default();
        incoming.add(
            None,
            BookmarkNode::Bookmark(BookmarkItem::new("Incoming", "https://example.com")),
        );
        studio.merge_session =
            Some(MergeSession::new(&studio.doc, vec![("incoming.html".into(), incoming)]).unwrap());
        studio
    }
    #[test]
    fn the_actual_charcoal_shell_renders_all_views_at_both_supported_sizes() {
        let context = egui::Context::default();
        crate::tessel::apply(&context, &rustmarks::ui_settings::UiSettings::default());
        let mut studio = merge_fixture();
        let before = studio.doc.clone();
        for size in [egui::vec2(1000., 650.), egui::vec2(1440., 900.)] {
            for (view, tab) in [
                (View::Explorer, Tab::Grid),
                (View::Explorer, Tab::Details),
                (View::Explorer, Tab::Diff),
                (View::Merge, Tab::Grid),
                (View::Organizer, Tab::Grid),
                (View::History, Tab::Grid),
                (View::Settings, Tab::Grid),
            ] {
                studio.view = view;
                studio.tab = tab;
                let output = context.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                        ..Default::default()
                    },
                    |ctx| studio.workspace(ctx),
                );
                assert!(!output.shapes.is_empty());
                assert_eq!(studio.doc, before);
                assert_eq!(
                    studio.merge_session.as_ref().unwrap().stats().unresolved,
                    1,
                    "Rendering must not select default conflict choices"
                );
            }
        }
    }
    #[test]
    fn refresh_failure_preserves_the_library_and_previous_review_choices() {
        let mut studio = merge_fixture();
        studio
            .merge_session
            .as_mut()
            .unwrap()
            .decide_all(rustmarks::merge::Resolution::KeepExisting);
        let before = studio.doc.clone();
        let stats = studio.merge_session.as_ref().unwrap().stats();
        let missing =
            std::env::temp_dir().join(format!("missing-rustmarks-{}.html", Uuid::new_v4()));
        studio.prepare_merge(vec![missing]);
        assert_eq!(studio.doc, before);
        let session = studio.merge_session.as_ref().unwrap();
        assert_eq!(session.stats(), stats);
        assert_eq!(
            session.decision(0),
            Some(rustmarks::merge::Resolution::KeepExisting)
        );
        assert!(studio.message.contains("unchanged"));
    }
    #[test]
    fn rejected_reviews_never_snapshot_or_mutate_the_library() {
        let temporary = TempHistory::new();
        let mut studio = merge_fixture();
        studio.history.directory = temporary.0.clone();
        let before = studio.doc.clone();
        studio.stage_merge();
        assert_eq!(studio.doc, before);
        assert!(studio.merge_session.is_some());
        assert!(!temporary.0.exists());
        studio
            .merge_session
            .as_mut()
            .unwrap()
            .decide_all(rustmarks::merge::Resolution::UseIncoming);
        studio
            .doc
            .root_folder
            .visit_mut(&mut |bookmark| bookmark.notes = "Later edit".into());
        let later = studio.doc.clone();
        studio.stage_merge();
        assert_eq!(studio.doc, later);
        assert!(!temporary.0.exists());
    }
    #[test]
    fn snapshot_failure_retains_the_ready_review_and_working_document() {
        let temporary = TempHistory::new();
        std::fs::write(&temporary.0, b"a file is not a history directory").unwrap();
        let mut studio = merge_fixture();
        studio.history.directory = temporary.0.clone();
        studio
            .merge_session
            .as_mut()
            .unwrap()
            .decide_all(rustmarks::merge::Resolution::UseIncoming);
        let before = studio.doc.clone();
        studio.stage_merge();
        assert_eq!(studio.doc, before);
        assert!(studio
            .merge_session
            .as_ref()
            .unwrap()
            .can_apply(&studio.doc));
        assert!(studio.message.contains("Snapshot failed"));
    }
    #[test]
    fn a_reviewed_merge_snapshots_before_staging_and_keeps_the_committed_baseline() {
        let temporary = TempHistory::new();
        let mut studio = merge_fixture();
        studio.history.directory = temporary.0.clone();
        studio
            .merge_session
            .as_mut()
            .unwrap()
            .decide_all(rustmarks::merge::Resolution::UseIncoming);
        let before = studio.doc.clone();
        studio.stage_merge();
        assert_eq!(studio.doc.items()[0].title, "Incoming");
        assert_eq!(studio.original, before);
        assert_eq!(studio.view, View::Explorer);
        assert_eq!(studio.tab, Tab::Diff);
        assert!(studio.merge_session.is_none());
        let snapshots = studio.history.list().unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(studio.history.restore(&snapshots[0]).unwrap(), before);
    }
    #[test]
    fn watch_dir_files_lists_only_bookmark_files() {
        let dir = std::env::temp_dir().join(format!("rustmarks-watch-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("export.html"), "<DL></DL>").unwrap();
        std::fs::write(dir.join("bookmarks.json"), "{}").unwrap();
        std::fs::write(dir.join("notes.txt"), "ignore me").unwrap();
        let files = watch_dir_files(&dir);
        assert_eq!(files.len(), 2);
        assert!(files.contains_key(&dir.join("export.html")));
        assert!(files.contains_key(&dir.join("bookmarks.json")));
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn workspace_panels_render_without_a_window_or_gpu() {
        let context = egui::Context::default();
        crate::tessel::apply(&context, &rustmarks::ui_settings::UiSettings::default());
        let mut studio = Studio::default();
        let mut first = None;
        for index in 0..1000 {
            let bookmark = BookmarkItem::new(
                format!("Bookmark {index}"),
                format!("https://example.com/{index}"),
            );
            if first.is_none() {
                first = Some(bookmark.id);
            }
            studio.doc.add(None, BookmarkNode::Bookmark(bookmark));
        }
        studio.selected.insert(first.unwrap());
        studio.original = studio.doc.clone();
        studio
            .doc
            .root_folder
            .visit_mut(&mut |b| b.notes = "Changed".into());
        for panel in 0..7 {
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1400., 900.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        studio.editor_tabs(ui);
                        ui.separator();
                        match panel {
                            0 => studio.grid(ui),
                            1 => studio.details(ui),
                            2 => studio.diff(ui),
                            3 => studio.organizer(ui),
                            4 => studio.merge_panel(ui),
                            5 => studio.history_panel(ui),
                            _ => studio.settings(ui),
                        }
                    });
                },
            );
        }
    }
}
