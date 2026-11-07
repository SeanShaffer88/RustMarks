use crate::models::*;
use uuid::Uuid;
#[derive(Clone)]
pub struct Conflict {
    pub existing: Uuid,
    pub incoming: BookmarkItem,
}
pub use crate::review_state::Resolution;
use crate::review_state::ReviewState;
pub struct MergePreview {
    pub document: BookmarkDocument,
    pub conflicts: Vec<Conflict>,
    pub added: usize,
    pub duplicates: usize,
}
pub fn normalize(raw: &str) -> String {
    let clean = crate::url_cleanup::clean(raw);
    match url::Url::parse(&clean) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => url.to_string(),
        _ => clean,
    }
}
pub fn preview(base: &BookmarkDocument, incoming: &BookmarkDocument) -> MergePreview {
    let mut result = MergePreview {
        document: base.clone(),
        conflicts: vec![],
        added: 0,
        duplicates: 0,
    };
    let mut index: std::collections::HashMap<String, BookmarkItem> = base
        .items()
        .into_iter()
        .map(|b| (normalize(&b.url), b))
        .collect();
    fn walk(
        node: &BookmarkNode,
        index: &mut std::collections::HashMap<String, BookmarkItem>,
        result: &mut MergePreview,
    ) -> Option<BookmarkNode> {
        match node {
            BookmarkNode::Bookmark(b) => {
                let key = normalize(&b.url);
                if let Some(old) = index.get(&key) {
                    result.duplicates += 1;
                    if old.title != b.title
                        || old.tags != b.tags
                        || old.notes != b.notes
                        || old.favicon_base64 != b.favicon_base64
                    {
                        result.conflicts.push(Conflict {
                            existing: old.id,
                            incoming: b.clone(),
                        });
                    }
                    None
                } else {
                    let mut b = b.clone();
                    b.id = Uuid::new_v4();
                    index.insert(key, b.clone());
                    result.added += 1;
                    Some(BookmarkNode::Bookmark(b))
                }
            }
            BookmarkNode::Folder {
                title,
                children,
                date_added,
                ..
            } => {
                let originally_empty = children.is_empty();
                let children: Vec<_> = children
                    .iter()
                    .filter_map(|n| walk(n, index, result))
                    .collect();
                if children.is_empty() && !originally_empty {
                    None
                } else {
                    Some(BookmarkNode::Folder {
                        id: Uuid::new_v4(),
                        title: title.clone(),
                        children,
                        date_added: *date_added,
                    })
                }
            }
        }
    }
    if let Some(n) = walk(&incoming.root_folder, &mut index, &mut result) {
        result.document.add(None, n);
    }
    result
}
pub fn resolve(doc: &mut BookmarkDocument, conflict: &Conflict, choice: Resolution) {
    if let Some(BookmarkNode::Bookmark(old)) = doc.root_folder.find_mut(conflict.existing) {
        match choice {
            Resolution::KeepExisting => {}
            Resolution::UseIncoming => {
                let id = old.id;
                *old = conflict.incoming.clone();
                old.id = id;
            }
            Resolution::Combine => {
                for tag in &conflict.incoming.tags {
                    if !old.tags.contains(tag) {
                        old.tags.push(tag.clone());
                    }
                }
                let incoming_lines: Vec<_> = conflict.incoming.notes.lines().collect();
                let existing_lines: Vec<_> = old.notes.lines().collect();
                let already_included = !incoming_lines.is_empty()
                    && existing_lines
                        .windows(incoming_lines.len())
                        .any(|lines| lines == incoming_lines);
                if !conflict.incoming.notes.is_empty() && !already_included {
                    if !old.notes.is_empty() {
                        old.notes.push('\n');
                    }
                    old.notes.push_str(&conflict.incoming.notes);
                }
                if old.favicon_base64.is_none() {
                    old.favicon_base64 = conflict.incoming.favicon_base64.clone();
                }
            }
        }
    }
}

/// File summaries contain real imported counts, never presentation placeholders.
#[derive(Clone, Debug)]
pub struct MergeSource {
    pub path: String,
    pub bookmark_count: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MergeStats {
    pub existing: usize,
    pub incoming: usize,
    pub total: usize,
    pub added: usize,
    pub duplicates: usize,
    pub conflicts: usize,
    pub unresolved: usize,
}
/// A staged, explicitly reviewed merge. Creating or reviewing it never changes the library.
pub struct MergeSession {
    base: BookmarkDocument,
    sources: Vec<MergeSource>,
    preview: MergePreview,
    decisions: ReviewState,
}
impl MergeSession {
    pub fn new(
        base: &BookmarkDocument,
        inputs: Vec<(String, BookmarkDocument)>,
    ) -> crate::parser::Result<Self> {
        base.validate()?;
        if inputs.is_empty() {
            return Err("Choose at least one incoming bookmark file".into());
        }
        let mut document = base.clone();
        let mut sources = vec![];
        let mut conflicts = vec![];
        let mut added = 0;
        let mut duplicates = 0;
        for (path, incoming) in inputs {
            incoming.validate()?;
            sources.push(MergeSource {
                path,
                bookmark_count: incoming.items().len(),
            });
            let next = preview(&document, &incoming);
            document = next.document;
            conflicts.extend(next.conflicts);
            added += next.added;
            duplicates += next.duplicates;
        }
        let decisions = ReviewState::new(conflicts.len());
        Ok(Self {
            base: base.clone(),
            sources,
            preview: MergePreview {
                document,
                conflicts,
                added,
                duplicates,
            },
            decisions,
        })
    }
    pub fn sources(&self) -> &[MergeSource] {
        &self.sources
    }
    pub fn preview(&self) -> &MergePreview {
        &self.preview
    }
    pub fn decision(&self, index: usize) -> Option<Resolution> {
        self.decisions.get(index)
    }
    pub fn set_decision(
        &mut self,
        index: usize,
        decision: Option<Resolution>,
    ) -> crate::parser::Result<()> {
        self.decisions.set(index, decision)?;
        Ok(())
    }
    pub fn decide_all(&mut self, decision: Resolution) {
        self.decisions.set_all(decision);
    }
    pub fn is_stale(&self, current: &BookmarkDocument) -> bool {
        current != &self.base
    }
    pub fn stats(&self) -> MergeStats {
        MergeStats {
            existing: self.base.items().len(),
            incoming: self
                .sources
                .iter()
                .map(|source| source.bookmark_count)
                .sum(),
            total: self.preview.document.items().len(),
            added: self.preview.added,
            duplicates: self.preview.duplicates,
            conflicts: self.preview.conflicts.len(),
            unresolved: self.decisions.remaining(),
        }
    }
    pub fn can_apply(&self, current: &BookmarkDocument) -> bool {
        self.decisions.validate(self.is_stale(current)).is_ok()
    }
    /// Return a new document only after validating the original and all decisions.
    /// Snapshots and disk writes remain the caller's responsibility.
    pub fn apply(&self, current: &BookmarkDocument) -> crate::parser::Result<BookmarkDocument> {
        self.decisions.validate(self.is_stale(current))?;
        let mut document = self.preview.document.clone();
        for (conflict, decision) in self.preview.conflicts.iter().zip(self.decisions.iter()) {
            if !matches!(
                document.root_folder.find_mut(conflict.existing),
                Some(BookmarkNode::Bookmark(_))
            ) {
                return Err("A conflict target is missing; refresh the preview".into());
            }
            resolve(
                &mut document,
                conflict,
                decision.ok_or("Missing conflict decision")?,
            );
        }
        document.validate()?;
        Ok(document)
    }
}
