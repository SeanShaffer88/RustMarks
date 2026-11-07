use crate::{
    merge_workspace::{Action as MergeAction, MergeWorkspace},
    ui_components as components, vscode_theme as theme,
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
    undo::UndoHistory,
};
use std::{collections::HashSet, path::PathBuf};
use uuid::Uuid;
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum View {
    Explorer,
    Organizer,
    Merge,
    History,
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
        }
    }
}
impl Studio {
    fn workspace(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("titlebar").show(ctx, |ui| self.toolbar(ui));
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::none()
                    .fill(theme::DEEP)
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
                                self.doc.items().len()
                            ))
                            .color(Color32::WHITE),
                        );
                    });
                });
            });
        egui::TopBottomPanel::bottom("notification").show(ctx, |ui| {
            ui.label(RichText::new(&self.message).small());
        });
        egui::SidePanel::left("activity")
            .exact_width(174.)
            .resizable(false)
            .frame(egui::Frame::none().fill(theme::DEEP).inner_margin(10.0_f32))
            .show(ctx, |ui| {
                ui.add_space(10.);
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(27., 27.), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 6.0_f32, theme::ACCENT);
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
                                .color(theme::MUTED),
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
                        .color(theme::MUTED),
                );
                ui.label(
                    RichText::new("Your bookmarks stay\non this device.")
                        .size(11.)
                        .color(theme::MUTED),
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
                    .fill(theme::EDITOR)
                    .inner_margin(24.0_f32),
            )
            .show(ctx, |ui| match self.view {
                View::Explorer => {
                    self.editor_tabs(ui);
                    ui.separator();
                    match self.tab {
                        Tab::Grid => self.grid(ui),
                        Tab::Details => self.details(ui),
                        Tab::Diff => self.diff(ui),
                    }
                }
                View::Organizer => self.organizer(ui),
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
                ui.selectable_value(&mut self.tab, Tab::Grid, "▤  Bookmarks"),
                ui.selectable_value(&mut self.tab, Tab::Details, "≡  Details"),
                ui.selectable_value(&mut self.tab, Tab::Diff, changes_label),
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
            Ok(()) => true,
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
                if ui.button("Open bookmarks…       Ctrl+O").clicked() {
                    ui.close_menu();
                    self.request_open();
                }
                if ui.button("Save                         Ctrl+S").clicked() {
                    ui.close_menu();
                    self.save(false);
                }
                if ui.button("Save as…       Ctrl+Shift+S").clicked() {
                    ui.close_menu();
                    self.save(true);
                }
                ui.separator();
                if ui.button("Merge files…").clicked() {
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
                    ("History", View::History, Tab::Grid),
                    ("Settings", View::Settings, Tab::Grid),
                ] {
                    if ui.button(label).clicked() {
                        self.navigate(view, tab);
                        ui.close_menu();
                    }
                }
            });
            ui.menu_button("Tools", |ui| {
                if ui.button("Scan categorization suggestions").clicked() {
                    if self.snapshot("suggest") {
                        heuristics::suggest_with(&mut self.doc, &self.rules);
                        self.view = View::Organizer;
                    }
                    ui.close_menu();
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("Settings & keyboard shortcuts").clicked() {
                    self.view = View::Settings;
                    ui.close_menu();
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new("●  Local workspace")
                        .size(11.)
                        .color(theme::GREEN),
                );
                if ui
                    .small_button("Save")
                    .on_hover_text("Commit staged changes · Ctrl+S")
                    .clicked()
                {
                    self.save(false);
                }
                if ui.small_button("Open").clicked() {
                    self.request_open();
                }
            });
        });
    }
    fn explorer(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("EXPLORER").small().color(theme::MUTED));
        ui.separator();
        ui.add(
            egui::TextEdit::singleline(&mut self.query)
                .hint_text("Search · tag:dev -broken")
                .desired_width(f32::INFINITY),
        );
        if ui
            .selectable_label(
                self.folder.is_none(),
                format!("All bookmarks  ({})", self.doc.items().len()),
            )
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
                    ui.text_edit_singleline(title);
                }
                if ui.button("Delete folder and contents").clicked()
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
        if ui.button("+ New folder").clicked() && self.snapshot("new-folder") {
            self.doc
                .add(self.folder, BookmarkNode::folder("New folder"));
        }
        ui.label(
            RichText::new("Changes are staged until you save.\nReview them in the Changes tab.")
                .small()
                .color(theme::MUTED),
        );
    }
    fn grid(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("+ Bookmark").clicked() && self.snapshot("add") {
                let b = BookmarkItem::new("Untitled bookmark", "https://");
                self.selected.clear();
                self.selected.insert(b.id);
                self.doc.add(self.folder, BookmarkNode::Bookmark(b));
            }
            ui.label(format!("{} selected", self.selected.len()));
            if ui.button("Select all filtered").clicked() {
                self.selected = self.visible().iter().map(|b| b.id).collect();
            }
            if ui.button("Delete selected").clicked()
                && !self.selected.is_empty()
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
            );
            if ui.button("Apply tag").clicked()
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
            if ui.button("Clean tracking URLs").clicked()
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
            let folders = self.doc.folders();
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
                });
            if ui
                .add_enabled(
                    self.move_target.is_some() && !self.selected.is_empty(),
                    egui::Button::new("Move selected"),
                )
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
                            ui.label(RichText::new(label).small().color(theme::MUTED));
                        });
                    }
                })
                .body(|body| {
                    body.rows(28., rows.len(), |mut row| {
                        let index = row.index();
                        let mut bookmark = rows[index].clone();
                        let before = bookmark.clone();
                        row.col(|ui| {
                            let selected = self.selected.contains(&bookmark.id);
                            if ui
                                .selectable_label(selected, format!("{:02}", index + 1))
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
                                        .desired_width(f32::INFINITY),
                                );
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
                                theme::GREEN,
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
    fn visible(&self) -> Vec<BookmarkItem> {
        fn collect(n: &BookmarkNode, id: Uuid) -> Option<&BookmarkNode> {
            if n.id() == id {
                return Some(n);
            }
            if let BookmarkNode::Folder { children, .. } = n {
                for c in children {
                    if let Some(found) = collect(c, id) {
                        return Some(found);
                    }
                }
            }
            None
        }
        let mut items = vec![];
        let node = self
            .folder
            .and_then(|id| collect(&self.doc.root_folder, id))
            .unwrap_or(&self.doc.root_folder);
        node.visit(&mut |b| {
            if heuristics::matches(b, &self.query) {
                items.push(b.clone());
            }
        });
        items
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
            ui.text_edit_singleline(&mut bookmark.title);
            ui.label("URL");
            ui.text_edit_singleline(&mut bookmark.url);
            if url::Url::parse(&bookmark.url).is_err() {
                ui.colored_label(theme::RED, "This URL is not valid.");
            }
            ui.label("Notes");
            ui.add(
                egui::TextEdit::multiline(&mut bookmark.notes)
                    .desired_rows(6)
                    .desired_width(f32::INFINITY),
            );
            ui.label("Tags (comma-separated)");
            let mut tags = bookmark.tags.join(", ");
            if ui.text_edit_singleline(&mut tags).changed() {
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
                if ui.button("Choose image…").clicked() {
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
                if ui.button("Clear favicon").clicked() {
                    bookmark.favicon_base64 = None;
                }
            });
            let mut icon = bookmark.favicon_base64.clone().unwrap_or_default();
            if ui
                .add(egui::TextEdit::singleline(&mut icon).hint_text("data:image/png;base64,…"))
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
                            ui.colored_label(theme::RED, format!("Cannot preview favicon: {e}"));
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
            ui.checkbox(&mut self.unified, "Unified view");
            if ui.button("Commit / Save").clicked() {
                self.save(false);
            }
            if ui.button("Revert all…").clicked() {
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
                Some(diff_view::Kind::Added) => theme::GREEN,
                Some(diff_view::Kind::Removed) => theme::RED,
                Some(_) => theme::ORANGE,
                None => theme::MUTED,
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
                    diff_view::Kind::Added => theme::GREEN,
                    diff_view::Kind::Removed => theme::RED,
                    _ => theme::ORANGE,
                };
                if self.unified {
                    ui.separator();
                    ui.colored_label(color, format!("{:?}  {}", change.kind, change.title));
                    if !change.before.is_empty() {
                        ui.colored_label(theme::RED, format!("− {}", change.before));
                    }
                    if !change.after.is_empty() {
                        ui.colored_label(theme::GREEN, format!("+ {}", change.after));
                    }
                } else {
                    egui::CollapsingHeader::new(
                        RichText::new(format!("{:?}  {}", change.kind, change.title)).color(color),
                    )
                    .id_source(("change", change.id))
                    .show(ui, |ui| {
                        ui.columns(2, |cols| {
                            cols[0].label(RichText::new(change.before).color(theme::RED));
                            cols[1].label(RichText::new(change.after).color(theme::GREEN));
                        });
                    });
                }
            }
        });
    }
    fn organizer(&mut self, ui: &mut egui::Ui) {
        ui.heading("Smart Organizer");
        ui.label("Local domain and keyword rules. No cloud services, no tracking.");
        ui.add_space(12.);
        ui.horizontal(|ui| {
            if ui.button("Scan suggestions").clicked() && self.snapshot("suggest") {
                heuristics::suggest_with(&mut self.doc, &self.rules);
                self.message = "Suggestions ready. Review below before organizing.".into();
            }
            if ui.button("Accept all → folders").clicked() && self.snapshot("organize") {
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
                    ui.colored_label(theme::GREEN, b.category_guess.as_deref().unwrap_or(""));
                    if ui.button("Accept").clicked() && self.snapshot("categorize") {
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
        ui.label("Compressed local snapshots. Restoring stages a working copy; it does not overwrite your file.");
        ui.horizontal(|ui| {
            if ui.button("Refresh").clicked() {
                match self.history.list() {
                    Ok(s) => self.snapshots = s,
                    Err(e) => self.message = e.to_string(),
                }
            }
            if ui.button("Create snapshot").clicked() && self.snapshot("manual") {
                self.snapshots = self.history.list().unwrap_or_default();
            }
        });
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
                        if ui.button("Inspect").clicked() {
                            inspect = Some(s.clone());
                        }
                        if ui.button("Restore").clicked() {
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
        ui.label("RustMarks 0.1.0 · Charcoal · Offline-first");
        ui.separator();
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
        if ui.button("Export Chrome / Edge JSON…").clicked() {
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
        ui.separator();
        ui.heading("Local categorization dictionary");
        ui.label("Earlier rules win equal scores. Domains match exact hosts and subdomains; keywords match whole tokens.");
        ui.add(
            egui::TextEdit::multiline(&mut self.rules_text)
                .code_editor()
                .desired_rows(14)
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            if ui.button("Validate and save rules").clicked() {
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
            if ui.button("Load default rules into editor").clicked() {
                self.rules_text =
                    serde_json::to_string_pretty(&RuleSet::default()).unwrap_or_default();
            }
        });
        ui.label("Ctrl+Z / Ctrl+Y undo or redo document edits when no text field is focused. Text fields keep their own undo while editing.");
        ui.label("For lossless editing, use RustMarks JSON as your working file and export HTML for your browser.");
    }
}
impl eframe::App for Studio {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.navigating_history = false;
        if !ctx.wants_keyboard_input() {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Z)) {
                self.undo_edit();
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Y)) {
                self.redo_edit();
            }
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.dirty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm_close = true;
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
                        if ui.button("Discard changes").clicked() && self.snapshot("before-discard")
                        {
                            self.doc = self.original.clone();
                            self.confirm_discard = false;
                            if self.pending_open {
                                self.pending_open = false;
                                self.open();
                            }
                        }
                        if ui.button("Keep editing").clicked() {
                            self.confirm_discard = false;
                            self.pending_open = false;
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
                        if ui.button("Save").clicked() {
                            self.save(false);
                            if !self.dirty() {
                                self.confirm_close = false;
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                        if ui.button("Discard and close").clicked() && self.snapshot("before-close")
                        {
                            self.original = self.doc.clone();
                            self.confirm_close = false;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui.button("Cancel").clicked() {
                            self.confirm_close = false;
                        }
                    });
                });
        }
        if self.doc != self.checkpoint {
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
            crate::vscode_theme::apply(&context);
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
        crate::vscode_theme::apply(&context);
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
    fn workspace_panels_render_without_a_window_or_gpu() {
        let context = egui::Context::default();
        crate::vscode_theme::apply(&context);
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
