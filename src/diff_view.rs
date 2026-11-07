use crate::models::*;
use std::collections::BTreeMap;
use uuid::Uuid;
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Added,
    Removed,
    Modified,
    Moved,
}
pub struct Change {
    pub id: Uuid,
    pub kind: Kind,
    pub title: String,
    pub before: String,
    pub after: String,
}
struct Entry<'a> {
    path: String,
    ancestors: Vec<Uuid>,
    position: usize,
    node: &'a BookmarkNode,
}
fn flatten<'a>(
    node: &'a BookmarkNode,
    path: &str,
    ancestors: &[Uuid],
    position: usize,
    out: &mut BTreeMap<Uuid, Entry<'a>>,
) {
    out.insert(
        node.id(),
        Entry {
            path: path.to_owned(),
            ancestors: ancestors.to_vec(),
            position,
            node,
        },
    );
    if let BookmarkNode::Folder {
        title, children, ..
    } = node
    {
        let mut parents = ancestors.to_vec();
        parents.push(node.id());
        for (index, child) in children.iter().enumerate() {
            flatten(child, &format!("{path}/{title}"), &parents, index, out);
        }
    }
}
pub fn changes(before: &BookmarkDocument, after: &BookmarkDocument) -> Vec<Change> {
    let mut a = BTreeMap::new();
    let mut b = BTreeMap::new();
    flatten(&before.root_folder, "", &[], 0, &mut a);
    flatten(&after.root_folder, "", &[], 0, &mut b);
    fn summary(e: &Entry<'_>) -> String {
        let path = &e.path;
        match e.node {
            BookmarkNode::Bookmark(b) => format!(
                "{path}/{}\n{}\nTags: {}\nNotes: {}\nCategory: {}\nAdded: {}\nFavicon: {}",
                b.title,
                b.url,
                b.tags.join(", "),
                b.notes,
                b.category_guess.as_deref().unwrap_or("—"),
                b.date_added,
                b.favicon_base64
                    .as_ref()
                    .map(|uri| {
                        use std::hash::{Hash, Hasher};
                        let mut hash = std::collections::hash_map::DefaultHasher::new();
                        uri.hash(&mut hash);
                        format!(
                            "embedded, {} bytes, fingerprint {:016x}",
                            uri.len(),
                            hash.finish()
                        )
                    })
                    .unwrap_or_else(|| "none".into())
            ),
            BookmarkNode::Folder {
                title, date_added, ..
            } => format!("{path}/{title}\nAdded: {date_added}"),
        }
    }
    let reordered = reordered_ids(&a, &b);
    let mut out = vec![];
    for (id, old) in &a {
        match b.get(id) {
            None => out.push(Change {
                id: *id,
                kind: Kind::Removed,
                title: old.node.title().into(),
                before: summary(old),
                after: String::new(),
            }),
            Some(new) => {
                let changed = match (old.node, new.node) {
                    (BookmarkNode::Bookmark(x), BookmarkNode::Bookmark(y)) => x != y,
                    (
                        BookmarkNode::Folder {
                            title: x,
                            date_added: dx,
                            ..
                        },
                        BookmarkNode::Folder {
                            title: y,
                            date_added: dy,
                            ..
                        },
                    ) => x != y || dx != dy,
                    _ => true,
                };
                // Parent identity catches moves between equally named folders.
                // Compare relative surviving sibling order so an insertion is not a move.
                let moved = old.ancestors != new.ancestors || reordered.contains(id);
                if changed || moved {
                    out.push(Change {
                        id: *id,
                        kind: if changed { Kind::Modified } else { Kind::Moved },
                        title: new.node.title().into(),
                        before: summary(old),
                        after: summary(new),
                    });
                }
            }
        }
    }
    for (id, new) in &b {
        if !a.contains_key(id) {
            out.push(Change {
                id: *id,
                kind: Kind::Added,
                title: new.node.title().into(),
                before: String::new(),
                after: summary(new),
            });
        }
    }
    out.sort_by(|a, b| {
        a.title
            .to_lowercase()
            .cmp(&b.title.to_lowercase())
            .then(a.id.cmp(&b.id))
    });
    out
}
fn reordered_ids(
    before: &BTreeMap<Uuid, Entry<'_>>,
    after: &BTreeMap<Uuid, Entry<'_>>,
) -> std::collections::HashSet<Uuid> {
    let mut groups = BTreeMap::<Vec<Uuid>, Vec<(usize, usize, Uuid)>>::new();
    for (id, old) in before {
        if let Some(new) = after.get(id) {
            if old.ancestors == new.ancestors {
                groups.entry(old.ancestors.clone()).or_default().push((
                    old.position,
                    new.position,
                    *id,
                ));
            }
        }
    }
    let mut changed = std::collections::HashSet::new();
    for mut siblings in groups.into_values() {
        siblings.sort_by_key(|e| e.0);
        let old: Vec<_> = siblings.iter().map(|e| e.2).collect();
        siblings.sort_by_key(|e| e.1);
        for (old, new) in old.into_iter().zip(siblings) {
            if old != new.2 {
                changed.insert(old);
                changed.insert(new.2);
            }
        }
    }
    changed
}
