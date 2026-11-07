//! Presentation only: no dialogs, writes, snapshots, or document mutation.
use crate::{ui_components as components, vscode_theme as theme};
use egui::{RichText, Ui};
use rustmarks::{
    merge::{MergeSession, Resolution},
    models::{BookmarkDocument, BookmarkItem},
};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    OpenLibrary,
    ChooseFiles,
    Refresh,
    Clear,
    Review,
}
pub struct MergeWorkspace {
    query: String,
    bulk: Resolution,
}
impl Default for MergeWorkspace {
    fn default() -> Self {
        Self {
            query: String::new(),
            bulk: Resolution::Combine,
        }
    }
}
fn decision_label(choice: Option<Resolution>) -> &'static str {
    match choice {
        None => "Choose resolution…",
        Some(Resolution::KeepExisting) => "Keep source A",
        Some(Resolution::UseIncoming) => "Prefer source B",
        Some(Resolution::Combine) => "Combine metadata",
    }
}
fn filename(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}
impl MergeWorkspace {
    pub fn reset(&mut self) {
        self.query.clear();
    }
    pub fn show(
        &mut self,
        ui: &mut Ui,
        library: &BookmarkDocument,
        mut session: Option<&mut MergeSession>,
    ) -> Action {
        let mut action = Action::None;
        let current_count = library.items().len();
        let source_a_name = library
            .source_path
            .as_deref()
            .map(filename)
            .unwrap_or_else(|| "Current library".into());
        let source_a_path = library
            .source_path
            .as_deref()
            .unwrap_or("Unsaved local workspace");
        let source_b_name = session
            .as_ref()
            .and_then(|review| {
                review.sources().first().map(|first| {
                    let name = filename(&first.path);
                    if review.sources().len() > 1 {
                        format!("{name} + {} more", review.sources().len() - 1)
                    } else {
                        name
                    }
                })
            })
            .unwrap_or_else(|| "Choose bookmark files".into());
        let source_b_path = session
            .as_ref()
            .map(|review| {
                review
                    .sources()
                    .iter()
                    .map(|source| source.path.as_str())
                    .collect::<Vec<_>>()
                    .join(" · ")
            })
            .unwrap_or_default();
        let source_b_count = session
            .as_ref()
            .map(|review| review.stats().incoming)
            .unwrap_or(0);
        egui::ScrollArea::vertical().id_source("merge-page").show(ui,|ui| {
            ui.label(RichText::new("WORKSPACE / MERGE").size(10.).color(theme::MUTED));
            ui.label(RichText::new("Merge Bookmark Files").size(23.).strong());
            ui.label(RichText::new("Bring your collections together. Review every conflict before saving.").size(12.).color(theme::MUTED));
            ui.add_space(12.);
            ui.columns(2,|columns| {
                if components::source_card(&mut columns[0],components::SourceCard{label:"Source A · your library",name:&source_a_name,path:source_a_path,count:current_count,action:"Open file…"}) {action=Action::OpenLibrary;}
                if components::source_card(&mut columns[1],components::SourceCard{label:"Source B · incoming files",name:&source_b_name,path:if source_b_path.is_empty(){"HTML or JSON · one or more files"}else{&source_b_path},count:source_b_count,action:"Choose files…"}) {action=Action::ChooseFiles;}
            });
            components::section(ui,"Duplicate Detection");
            let stats=session.as_ref().map(|review|review.stats());
            let (duplicates,conflicts,added)=stats.map(|stats|(stats.duplicates,stats.conflicts,stats.added)).unwrap_or((0,0,0));
            components::card(ui,|ui|ui.columns(4,|columns| {
                components::metric(&mut columns[0],duplicates,"Duplicate bookmarks",theme::ACCENT);
                components::metric(&mut columns[1],duplicates.saturating_sub(conflicts),"Matching metadata",theme::ORANGE);
                components::metric(&mut columns[2],conflicts,"Metadata conflicts",theme::GREEN);
                components::metric(&mut columns[3],added,"New bookmarks",theme::RED);
            }));
            components::section(ui,"Resolve Conflicts");
            if let Some(review)=session.as_deref_mut() {self.conflicts(ui,review);} else {
                components::card(ui,|ui| {
                    ui.add_space(14.);ui.vertical_centered(|ui| {
                        ui.label(RichText::new("Two collections. One organized library.").size(16.).strong());
                        ui.label(RichText::new("Choose incoming files to find duplicates and compare metadata.").color(theme::MUTED));
                        if ui.button("Choose source B…").on_hover_text("Pick one or more bookmark files to merge in. Nothing changes until you review.").clicked(){action=Action::ChooseFiles;}
                    });ui.add_space(14.);
                });
            }
            components::section(ui,"Merge Preview");
            let stats=session.as_ref().map(|review|review.stats());
            components::card(ui,|ui|ui.columns(5,|columns| {
                components::metric(&mut columns[0],stats.map(|s|s.total).unwrap_or(current_count),"Total bookmarks",theme::ACCENT);
                components::metric(&mut columns[1],stats.map(|s|s.existing).unwrap_or(current_count),"Already in library",theme::ORANGE);
                components::metric(&mut columns[2],added,"Will be added",theme::GREEN);
                components::metric(&mut columns[3],duplicates,"Duplicates skipped",theme::GREEN);
                components::metric(&mut columns[4],stats.map(|s|s.unresolved).unwrap_or(0),"Choices remaining",theme::ORANGE);
            }));
            ui.add_space(12.);
            if let Some(review)=session.as_ref() {
                if review.is_stale(library){ui.colored_label(theme::ORANGE,"Your library changed. Refresh the incoming files before applying.");}
                else if review.stats().unresolved>0 {ui.colored_label(theme::ORANGE,"Choose a resolution for every conflict, or explicitly apply one choice to all.");}
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new("Staged locally. Nothing is written until you save.").size(11.).color(theme::MUTED));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| {
                    if components::primary(ui,"Review Changes  →",session.as_ref().is_some_and(|review|review.can_apply(library))).clicked(){action=Action::Review;}
                    if ui.add_enabled(session.is_some(),egui::Button::new("Clear")).clicked(){action=Action::Clear;}
                    if ui.add_enabled(session.is_some(),egui::Button::new("Refresh")).on_hover_text("Reload the same incoming paths and build a fresh preview").clicked(){action=Action::Refresh;}
                });
            });
        });
        action
    }
    fn conflicts(&mut self, ui: &mut Ui, review: &mut MergeSession) {
        if review.preview().conflicts.is_empty() {
            components::card(ui, |ui| {
                ui.colored_label(theme::GREEN, "No metadata conflicts to resolve.");
                ui.label(RichText::new("Matching duplicates are skipped automatically; folder structure is preserved.").small().color(theme::MUTED));
            });
            return;
        }
        let existing: HashMap<_, _> = review
            .preview()
            .document
            .items()
            .into_iter()
            .map(|bookmark| (bookmark.id, bookmark))
            .collect();
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text("Filter conflicts…")
                    .desired_width(190.),
            )
            .on_hover_text(
                "Filter the conflict list by title or URL. Hidden conflicts still need a decision.",
            );
            egui::ComboBox::from_id_source("merge-bulk-choice")
                .selected_text(decision_label(Some(self.bulk)))
                .show_ui(ui, |ui| {
                    for choice in [
                        Resolution::KeepExisting,
                        Resolution::UseIncoming,
                        Resolution::Combine,
                    ] {
                        ui.selectable_value(&mut self.bulk, choice, decision_label(Some(choice)));
                    }
                })
                .response
                .on_hover_text(
                    "Choice used by \"Apply to all\". Nothing is applied until you click it.",
                );
            if ui
                .button(format!(
                    "Apply to all {} conflicts",
                    review.preview().conflicts.len()
                ))
                .on_hover_text(
                    "Set every conflict to the choice at left, replacing earlier decisions",
                )
                .clicked()
            {
                review.decide_all(self.bulk);
            }
        });
        let query = self.query.trim().to_lowercase();
        let visible: Vec<_> = review
            .preview()
            .conflicts
            .iter()
            .enumerate()
            .filter(|(_, conflict)| {
                let old = existing.get(&conflict.existing);
                query.is_empty()
                    || format!(
                        "{} {} {} {} {}",
                        conflict.incoming.title,
                        conflict.incoming.url,
                        conflict.incoming.notes,
                        conflict.incoming.tags.join(" "),
                        old.map(|old| format!(
                            "{} {} {} {}",
                            old.title,
                            old.url,
                            old.notes,
                            old.tags.join(" ")
                        ))
                        .unwrap_or_default()
                    )
                    .to_lowercase()
                    .contains(&query)
            })
            .map(|(index, _)| index)
            .collect();
        ui.label(
            RichText::new(format!(
                "{} of {} conflicts",
                visible.len(),
                review.preview().conflicts.len()
            ))
            .small()
            .color(theme::MUTED),
        );
        components::card(ui, |ui| {
            if visible.is_empty() {
                ui.label("No conflicts match this filter. Hidden conflicts still need decisions.");
                return;
            }
            egui::ScrollArea::horizontal()
                .id_source("conflict-horizontal")
                .show(ui, |ui| {
                    ui.set_min_width(700.);
                    use egui_extras::{Column, TableBuilder};
                    TableBuilder::new(ui)
                        .striped(true)
                        .resizable(true)
                        .max_scroll_height(240.)
                        .column(Column::initial(145.).at_least(100.))
                        .column(Column::remainder().at_least(150.))
                        .column(Column::remainder().at_least(150.))
                        .column(Column::exact(175.))
                        .header(25., |mut header| {
                            for title in ["Title", "Source A", "Source B", "Resolution"] {
                                header.col(|ui| {
                                    ui.label(RichText::new(title).small().color(theme::MUTED));
                                });
                            }
                        })
                        .body(|body| {
                            body.rows(38., visible.len(), |mut row| {
                                let index = visible[row.index()];
                                let conflict = &review.preview().conflicts[index];
                                row.col(|ui| {
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(&conflict.incoming.title).strong(),
                                        )
                                        .truncate(),
                                    )
                                    .on_hover_text(&conflict.incoming.title);
                                });
                                row.col(|ui| {
                                    if let Some(old) = existing.get(&conflict.existing) {
                                        url_cell(ui, old);
                                    } else {
                                        ui.label("Missing target");
                                    }
                                });
                                row.col(|ui| {
                                    url_cell(ui, &conflict.incoming);
                                });
                                row.col(|ui| {
                                    let original = review.decision(index);
                                    let mut decision = original;
                                    let color = if decision.is_some() {
                                        theme::GREEN
                                    } else {
                                        theme::ORANGE
                                    };
                                    egui::ComboBox::from_id_source(("merge-decision", index))
                                        .width(155.)
                                        .selected_text(
                                            RichText::new(decision_label(decision))
                                                .small()
                                                .color(color),
                                        )
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(
                                                &mut decision,
                                                None,
                                                "Choose resolution…",
                                            );
                                            for choice in [
                                                Resolution::KeepExisting,
                                                Resolution::UseIncoming,
                                                Resolution::Combine,
                                            ] {
                                                ui.selectable_value(
                                                    &mut decision,
                                                    Some(choice),
                                                    decision_label(Some(choice)),
                                                );
                                            }
                                        })
                                        .response
                                        .on_hover_text("How to resolve this conflict: keep the existing entry, prefer the incoming one, or combine metadata");
                                    if decision != original {
                                        let _ = review.set_decision(index, decision);
                                    }
                                });
                            })
                        });
                });
        });
    }
}
fn url_cell(ui: &mut Ui, bookmark: &BookmarkItem) {
    ui.add(egui::Label::new(RichText::new(&bookmark.url).size(11.).color(theme::MUTED)).truncate())
        .on_hover_text(format!(
            "{}\n{}\nTags: {}\n{}",
            bookmark.title,
            bookmark.url,
            bookmark.tags.join(", "),
            bookmark.notes
        ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustmarks::models::BookmarkNode;
    #[test]
    fn filters_cannot_implicitly_resolve_hidden_conflicts_or_change_the_library() {
        let mut library = BookmarkDocument::default();
        for title in ["First", "Second"] {
            library.add(
                None,
                BookmarkNode::Bookmark(BookmarkItem::new(
                    title,
                    format!("https://{}.test", title.to_lowercase()),
                )),
            );
        }
        let mut incoming = library.clone();
        incoming
            .root_folder
            .visit_mut(&mut |bookmark| bookmark.notes = "Incoming notes".into());
        let mut session =
            MergeSession::new(&library, vec![("incoming.html".into(), incoming)]).unwrap();
        let before = library.clone();
        let context = egui::Context::default();
        crate::vscode_theme::apply(&context);
        let mut workspace = MergeWorkspace::default();
        for query in ["First", "nothing matches", ""] {
            workspace.query = query.into();
            let mut action = Action::None;
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(900., 700.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        action = workspace.show(ui, &library, Some(&mut session));
                    });
                },
            );
            assert_eq!(action, Action::None);
            assert_eq!(library, before);
            assert_eq!(session.stats().unresolved, 2);
            assert!(!session.can_apply(&library));
        }
    }
    #[test]
    fn empty_incoming_state_renders_without_fabricated_sources() {
        let library = BookmarkDocument::default();
        let context = egui::Context::default();
        let mut workspace = MergeWorkspace::default();
        let _ = context.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert_eq!(workspace.show(ui, &library, None), Action::None);
            });
        });
        assert_eq!(library.items().len(), 0);
    }
}
