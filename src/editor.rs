use crate::vscode_theme as theme;
use egui::{Color32, RichText};
use rustmarks::{
    diff_view, heuristics,
    history::{History, Snapshot},
    merge::{self, Resolution},
    models::*,
    parser,
};
use std::{collections::HashSet, path::PathBuf};
use uuid::Uuid;
#[derive(PartialEq, Clone, Copy)]
enum View {
    Explorer,
    Organizer,
    Merge,
    History,
    Settings,
}
#[derive(PartialEq)]
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
    pending_merge: Option<merge::MergePreview>,
    merge_base: Option<BookmarkDocument>,
    resolutions: Vec<Resolution>,
    unified: bool,
    confirm_discard: bool,
    pending_open: bool,
    confirm_close: bool,
}
impl Default for Studio {
    fn default() -> Self {
        let doc = BookmarkDocument::default();
        Self {
            original: doc.clone(),
            doc,
            view: View::Explorer,
            tab: Tab::Grid,
            query: String::new(),
            folder: None,
            selected: HashSet::new(),
            anchor: None,
            bulk_tag: String::new(),
            message: "Ready. Open a bookmark file to get started.".into(),
            history: History::default(),
            snapshots: vec![],
            pending_merge: None,
            merge_base: None,
            resolutions: vec![],
            unified: false,
            confirm_discard: false,
            pending_open: false,
            confirm_close: false,
        }
    }
}
impl Studio {
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
            match parser::load(&path) {
                Ok(doc) => {
                    self.original = doc.clone();
                    self.doc = doc;
                    self.folder = None;
                    self.selected.clear();
                    self.message = "File opened locally. No data leaves this device.".into();
                }
                Err(e) => self.message = format!("Open failed: {e}"),
            }
        }
    }
    fn save(&mut self, save_as: bool) {
        let path = if save_as || self.doc.source_path.is_none() {
            rfd::FileDialog::new()
                .add_filter("RustMarks JSON (all metadata)", &["json"])
                .add_filter("Netscape bookmarks", &["html"])
                .set_file_name("bookmarks.json")
                .save_file()
        } else {
            self.doc.source_path.as_ref().map(PathBuf::from)
        };
        if let Some(path) = path {
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
            match parser::save(&doc, &path) {
                Ok(()) => {
                    self.doc = doc.clone();
                    self.original = doc;
                    self.message = format!("Saved {}", path.display());
                }
                Err(e) => self.message = format!("Save failed: {e}"),
            }
        }
    }
    fn begin_merge(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Bookmarks", &["html", "htm", "json"])
            .pick_files()
        {
            let mut accumulated = self.doc.clone();
            let mut conflicts = vec![];
            let mut added = 0;
            let mut duplicates = 0;
            for path in paths {
                match parser::load(&path) {
                    Ok(incoming) => {
                        let p = merge::preview(&accumulated, &incoming);
                        accumulated = p.document;
                        conflicts.extend(p.conflicts);
                        added += p.added;
                        duplicates += p.duplicates;
                    }
                    Err(e) => {
                        self.message = format!("Merge import failed (no changes applied): {e}");
                        return;
                    }
                }
            }
            self.merge_base = Some(self.doc.clone());
            self.resolutions = vec![Resolution::Combine; conflicts.len()];
            self.pending_merge = Some(merge::MergePreview {
                document: accumulated,
                conflicts,
                added,
                duplicates,
            });
            self.view = View::Merge;
        }
    }
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("R").strong().color(theme::ACCENT).size(22.));
            ui.label(RichText::new("RustMarks").strong());
            ui.label(RichText::new("BOOKMARK STUDIO").small().color(theme::MUTED));
            ui.separator();
            if ui.button("Open…").on_hover_text("Ctrl+O").clicked() {
                if self.dirty() {
                    self.pending_open = true;
                    self.confirm_discard = true;
                } else {
                    self.open();
                }
            }
            if ui
                .button("Save")
                .on_hover_text("Ctrl+S — commit staged changes")
                .clicked()
            {
                self.save(false);
            }
            if ui.button("Save as…").clicked() {
                self.save(true);
            }
            if ui.button("Merge files…").clicked() {
                self.begin_merge();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new("●  OFFLINE").small().color(theme::GREEN));
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
        if ui.button("+ New folder").clicked() {
            if self.snapshot("new-folder") {
                self.doc
                    .add(self.folder, BookmarkNode::folder("New folder"));
            }
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
                        b.url = merge::normalize(&b.url)
                    }
                });
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
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("bookmark-grid")
                    .striped(true)
                    .min_col_width(40.)
                    .spacing([12., 7.])
                    .show(ui, |ui| {
                        for name in ["", "TITLE", "URL", "TAGS", "NOTES", "CATEGORY"] {
                            ui.label(RichText::new(name).small().color(theme::MUTED));
                        }
                        ui.end_row();
                        for (index, mut b) in rows.into_iter().enumerate() {
                            let before = b.clone();
                            let selected = self.selected.contains(&b.id);
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
                                        self.selected.remove(&b.id);
                                    } else {
                                        self.selected.insert(b.id);
                                    }
                                    self.anchor = Some(index);
                                } else {
                                    self.selected.clear();
                                    self.selected.insert(b.id);
                                    self.anchor = Some(index);
                                }
                            }
                            ui.add(egui::TextEdit::singleline(&mut b.title).desired_width(190.));
                            ui.scope(|ui| {
                                ui.visuals_mut().override_text_color = Some(theme::ORANGE);
                                ui.add(egui::TextEdit::singleline(&mut b.url).desired_width(260.));
                            });
                            let mut tags = b.tags.join(", ");
                            if ui
                                .add(egui::TextEdit::singleline(&mut tags).desired_width(140.))
                                .changed()
                            {
                                b.tags = tags
                                    .split(',')
                                    .map(str::trim)
                                    .filter(|s| !s.is_empty())
                                    .map(str::to_owned)
                                    .collect();
                            }
                            ui.add(egui::TextEdit::singleline(&mut b.notes).desired_width(180.));
                            ui.colored_label(
                                theme::GREEN,
                                b.category_guess.as_deref().unwrap_or("—"),
                            );
                            ui.end_row();
                            if b != before {
                                if let Some(BookmarkNode::Bookmark(target)) =
                                    self.doc.root_folder.find_mut(b.id)
                                {
                                    *target = b;
                                }
                            }
                        }
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
        let Some(id) = self.selected.iter().next().copied() else {
            ui.label("Select a row in the grid to inspect its details.");
            return;
        };
        if let Some(BookmarkNode::Bookmark(b)) = self.doc.root_folder.find_mut(id) {
            ui.heading("Bookmark details");
            ui.label("Title");
            ui.text_edit_singleline(&mut b.title);
            ui.label("URL");
            ui.text_edit_singleline(&mut b.url);
            if url::Url::parse(&b.url).is_err() {
                ui.colored_label(theme::RED, "This URL is not valid.");
            }
            ui.label("Notes");
            ui.add(
                egui::TextEdit::multiline(&mut b.notes)
                    .desired_rows(6)
                    .desired_width(f32::INFINITY),
            );
            ui.label("Tags (comma-separated)");
            let mut tags = b.tags.join(", ");
            if ui.text_edit_singleline(&mut tags).changed() {
                b.tags = tags
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect();
            }
            ui.label("Favicon data URI (stored locally; never fetched)");
            let mut icon = b.favicon_base64.clone().unwrap_or_default();
            if ui.text_edit_singleline(&mut icon).changed() {
                b.favicon_base64 = if icon.is_empty() { None } else { Some(icon) };
            }
            ui.label(format!("Added: {} · ID: {}", b.date_added, b.id));
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
        egui::ScrollArea::vertical().show(ui, |ui| {
            for c in changes {
                let color = match c.kind {
                    diff_view::Kind::Added => theme::GREEN,
                    diff_view::Kind::Removed => theme::RED,
                    _ => theme::ORANGE,
                };
                ui.separator();
                ui.colored_label(color, format!("{:?}  {}", c.kind, c.title));
                if self.unified {
                    if !c.before.is_empty() {
                        ui.colored_label(theme::RED, format!("− {}", c.before));
                    }
                    if !c.after.is_empty() {
                        ui.colored_label(theme::GREEN, format!("+ {}", c.after));
                    }
                } else {
                    ui.columns(2, |cols| {
                        cols[0].label(RichText::new(c.before).color(theme::RED));
                        cols[1].label(RichText::new(c.after).color(theme::GREEN));
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
                heuristics::suggest(&mut self.doc);
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
        ui.heading("Merge Studio");
        ui.label("Combine files, normalize tracking URLs, and resolve metadata conflicts before applying.");
        if ui.button("Choose bookmark files…").clicked() {
            self.begin_merge();
        }
        let mut apply = false;
        let mut cancel = false;
        if let Some(preview) = &self.pending_merge {
            ui.label(format!(
                "{} new bookmarks · {} duplicates · {} conflicts",
                preview.added,
                preview.duplicates,
                preview.conflicts.len()
            ));
            ui.horizontal(|ui| {
                apply = ui.button("Apply merge to working copy").clicked();
                cancel = ui.button("Cancel preview").clicked();
            });
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, c) in preview.conflicts.iter().enumerate() {
                    ui.separator();
                    ui.label(RichText::new(&c.incoming.url).color(theme::ORANGE));
                    if let Some(old) = self.doc.items().into_iter().find(|b| b.id == c.existing) {
                        ui.label(format!(
                            "Existing: {}\nTags: {}\n{}",
                            old.title,
                            old.tags.join(", "),
                            old.notes
                        ));
                    }
                    ui.label(format!(
                        "Incoming: {}\nTags: {}\n{}",
                        c.incoming.title,
                        c.incoming.tags.join(", "),
                        c.incoming.notes
                    ));
                    ui.horizontal(|ui| {
                        ui.radio_value(
                            &mut self.resolutions[i],
                            Resolution::KeepExisting,
                            "Keep existing",
                        );
                        ui.radio_value(
                            &mut self.resolutions[i],
                            Resolution::UseIncoming,
                            "Use incoming",
                        );
                        ui.radio_value(
                            &mut self.resolutions[i],
                            Resolution::Combine,
                            "Combine tags + notes",
                        );
                    });
                }
            });
        } else {
            ui.add_space(24.);
            ui.label(
                "No pending merge. Your active document is unchanged until you apply a preview.",
            );
        }
        if cancel {
            self.pending_merge = None;
        }
        if apply && self.merge_base.as_ref() != Some(&self.doc) {
            self.message = "The working copy changed after this preview. Choose files again to refresh the merge.".into();
            return;
        }
        if apply && self.snapshot("before-merge") {
            if let Some(mut p) = self.pending_merge.take() {
                for (c, r) in p.conflicts.iter().zip(&self.resolutions) {
                    merge::resolve(&mut p.document, c, *r);
                }
                self.doc = p.document;
                self.view = View::Explorer;
                self.tab = Tab::Diff;
            }
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
        egui::ScrollArea::vertical().show(ui, |ui| {
            for s in &self.snapshots {
                ui.horizontal(|ui| {
                    if ui.button("Restore").clicked() {
                        restore = Some(s.clone());
                    }
                    ui.label(&s.label);
                });
            }
        });
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
        ui.label("RustMarks 0.1.0 · Dark Modern · Offline-first");
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
                    Ok(data) => match std::fs::write(&path, data) {
                        Ok(()) => self.message = format!("Exported {}", path.display()),
                        Err(e) => self.message = e.to_string(),
                    },
                    Err(e) => self.message = e.to_string(),
                }
            }
        }
        ui.label("For lossless editing, use RustMarks JSON as your working file and export HTML for your browser.");
    }
}
impl eframe::App for Studio {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
        egui::TopBottomPanel::top("titlebar").show(ctx, |ui| self.toolbar(ui));
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::none()
                    .fill(theme::ACCENT)
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
            .exact_width(55.)
            .resizable(false)
            .show(ctx, |ui| {
                for (view, icon, label) in [
                    (View::Explorer, "▤", "Explorer"),
                    (View::Organizer, "✧", "Smart Organizer"),
                    (View::Merge, "⇄", "Merge Studio"),
                    (View::History, "◷", "Version History"),
                    (View::Settings, "⚙", "Settings"),
                ] {
                    if ui
                        .add_sized(
                            [38., 44.],
                            egui::SelectableLabel::new(
                                self.view == view,
                                RichText::new(icon).size(24.),
                            ),
                        )
                        .on_hover_text(label)
                        .clicked()
                    {
                        self.view = view;
                        if view == View::History {
                            match self.history.list() {
                                Ok(s) => self.snapshots = s,
                                Err(e) => self.message = e.to_string(),
                            }
                        }
                    }
                }
            });
        if self.view == View::Explorer {
            egui::SidePanel::left("explorer")
                .default_width(235.)
                .width_range(180. ..=400.)
                .show(ctx, |ui| self.explorer(ui));
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(theme::EDITOR).inner_margin(16.))
            .show(ctx, |ui| match self.view {
                View::Explorer => {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.tab, Tab::Grid, "▤  Bookmarks");
                        ui.selectable_value(&mut self.tab, Tab::Details, "≡  Details");
                        ui.selectable_value(
                            &mut self.tab,
                            Tab::Diff,
                            format!("⇄  Changes{}", if self.dirty() { " ●" } else { "" }),
                        );
                    });
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
                View::Settings => self.settings(ui),
            });
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
    }
}
