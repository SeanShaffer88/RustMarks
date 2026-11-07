use crate::models::*;
use std::collections::BTreeMap;
use uuid::Uuid;
#[derive(Debug, PartialEq)]
pub enum Kind {
    Added,
    Removed,
    Modified,
    Moved,
}
pub struct Change {
    pub kind: Kind,
    pub title: String,
    pub before: String,
    pub after: String,
}
fn flatten(node: &BookmarkNode, path: &str, out: &mut BTreeMap<Uuid, (String, BookmarkNode)>) {
    out.insert(node.id(), (path.to_owned(), node.clone()));
    if let BookmarkNode::Folder {
        title, children, ..
    } = node
    {
        for c in children {
            flatten(c, &format!("{path}/{title}"), out);
        }
    }
}
pub fn changes(before: &BookmarkDocument, after: &BookmarkDocument) -> Vec<Change> {
    let mut a = BTreeMap::new();
    let mut b = BTreeMap::new();
    flatten(&before.root_folder, "", &mut a);
    flatten(&after.root_folder, "", &mut b);
    fn summary(path: &str, n: &BookmarkNode) -> String {
        match n {
            BookmarkNode::Bookmark(b) => format!(
                "{path}/{}\n{}\nTags: {}\n{}",
                b.title,
                b.url,
                b.tags.join(", "),
                b.notes
            ),
            _ => format!("{path}/{}", n.title()),
        }
    }
    let mut out = vec![];
    for (id, (path, n)) in &a {
        match b.get(id) {
            None => out.push(Change {
                kind: Kind::Removed,
                title: n.title().to_owned(),
                before: summary(path, n),
                after: String::new(),
            }),
            Some((p, m)) => {
                let content_changed = match (n, m) {
                    (BookmarkNode::Bookmark(x), BookmarkNode::Bookmark(y)) => x != y,
                    (
                        BookmarkNode::Folder { title: x, .. },
                        BookmarkNode::Folder { title: y, .. },
                    ) => x != y,
                    _ => true,
                };
                if content_changed || p != path {
                    out.push(Change {
                        kind: if content_changed {
                            Kind::Modified
                        } else {
                            Kind::Moved
                        },
                        title: m.title().to_owned(),
                        before: summary(path, n),
                        after: summary(p, m),
                    });
                }
            }
        }
    }
    for (id, (path, n)) in &b {
        if !a.contains_key(id) {
            out.push(Change {
                kind: Kind::Added,
                title: n.title().to_owned(),
                before: String::new(),
                after: summary(path, n),
            });
        }
    }
    out
}
