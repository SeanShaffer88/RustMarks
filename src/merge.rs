use crate::models::*;
use uuid::Uuid;
#[derive(Clone)]
pub struct Conflict {
    pub existing: Uuid,
    pub incoming: BookmarkItem,
}
#[derive(Clone, Copy, PartialEq)]
pub enum Resolution {
    KeepExisting,
    UseIncoming,
    Combine,
}
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
                        old.notes.push_str("\n");
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
