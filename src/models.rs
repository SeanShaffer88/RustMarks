use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct BookmarkItem {
    pub id: Uuid,
    pub title: String,
    pub url: String,
    pub favicon_base64: Option<String>,
    pub notes: String,
    pub tags: Vec<String>,
    pub date_added: i64,
    pub category_guess: Option<String>,
}
impl BookmarkItem {
    pub fn new(title: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            url: url.into(),
            favicon_base64: None,
            notes: String::new(),
            tags: vec![],
            date_added: chrono::Utc::now().timestamp(),
            category_guess: None,
        }
    }
}
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum BookmarkNode {
    Folder {
        id: Uuid,
        title: String,
        children: Vec<BookmarkNode>,
        date_added: i64,
    },
    Bookmark(BookmarkItem),
}
impl BookmarkNode {
    pub fn folder(title: impl Into<String>) -> Self {
        Self::Folder {
            id: Uuid::new_v4(),
            title: title.into(),
            children: vec![],
            date_added: chrono::Utc::now().timestamp(),
        }
    }
    pub fn id(&self) -> Uuid {
        match self {
            Self::Folder { id, .. } => *id,
            Self::Bookmark(b) => b.id,
        }
    }
    pub fn title(&self) -> &str {
        match self {
            Self::Folder { title, .. } => title,
            Self::Bookmark(b) => &b.title,
        }
    }
    pub fn children_mut(&mut self) -> Option<&mut Vec<Self>> {
        match self {
            Self::Folder { children, .. } => Some(children),
            _ => None,
        }
    }
    pub fn visit(&self, f: &mut impl FnMut(&BookmarkItem)) {
        match self {
            Self::Bookmark(b) => f(b),
            Self::Folder { children, .. } => children.iter().for_each(|n| n.visit(f)),
        }
    }
    pub fn visit_mut(&mut self, f: &mut impl FnMut(&mut BookmarkItem)) {
        match self {
            Self::Bookmark(b) => f(b),
            Self::Folder { children, .. } => children.iter_mut().for_each(|n| n.visit_mut(f)),
        }
    }
    pub fn find_mut(&mut self, id: Uuid) -> Option<&mut Self> {
        if self.id() == id {
            return Some(self);
        }
        self.children_mut()?.iter_mut().find_map(|n| n.find_mut(id))
    }
    pub fn remove(&mut self, ids: &std::collections::HashSet<Uuid>) {
        if let Some(children) = self.children_mut() {
            children.retain(|n| !ids.contains(&n.id()));
            for child in children {
                child.remove(ids);
            }
        }
    }
}
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct BookmarkDocument {
    pub version: u32,
    pub root_folder: BookmarkNode,
    pub source_path: Option<String>,
}
impl Default for BookmarkDocument {
    fn default() -> Self {
        Self {
            version: 1,
            root_folder: BookmarkNode::folder("Bookmarks"),
            source_path: None,
        }
    }
}
impl BookmarkDocument {
    pub fn items(&self) -> Vec<BookmarkItem> {
        let mut items = vec![];
        self.root_folder.visit(&mut |b| items.push(b.clone()));
        items
    }
    pub fn add(&mut self, folder: Option<Uuid>, node: BookmarkNode) {
        let target = folder
            .and_then(|id| self.root_folder.find_mut(id))
            .and_then(|n| n.children_mut());
        if let Some(children) = target {
            children.push(node);
        } else if let Some(children) = self.root_folder.children_mut() {
            children.push(node);
        }
    }
}

impl BookmarkDocument {
    /// Reject corrupt native trees before traversal or mutation can lose data.
    pub fn validate(&self) -> crate::parser::Result<()> {
        if self.version != 1 {
            return Err("Unsupported RustMarks document version".into());
        }
        if !matches!(self.root_folder, BookmarkNode::Folder { .. }) {
            return Err("Document root must be a folder".into());
        }
        let mut ids = std::collections::HashSet::new();
        let mut pending = vec![(&self.root_folder, 0)];
        while let Some((node, depth)) = pending.pop() {
            if depth > 128 {
                return Err("Folder nesting exceeds 128 levels".into());
            }
            if !ids.insert(node.id()) {
                return Err("Duplicate bookmark or folder IDs".into());
            }
            if let BookmarkNode::Folder { children, .. } = node {
                pending.extend(children.iter().map(|n| (n, depth + 1)));
            }
        }
        Ok(())
    }
}

impl BookmarkDocument {
    pub fn folders(&self) -> Vec<(Uuid, String)> {
        fn walk(node: &BookmarkNode, prefix: &str, out: &mut Vec<(Uuid, String)>) {
            if let BookmarkNode::Folder {
                id,
                title,
                children,
                ..
            } = node
            {
                let path = if prefix.is_empty() {
                    title.clone()
                } else {
                    format!("{prefix}/{title}")
                };
                out.push((*id, path.clone()));
                for child in children {
                    walk(child, &path, out);
                }
            }
        }
        let mut out = vec![];
        walk(&self.root_folder, "", &mut out);
        out
    }
    pub fn move_bookmarks(
        &mut self,
        ids: &std::collections::HashSet<Uuid>,
        folder: Uuid,
    ) -> crate::parser::Result<()> {
        if !matches!(
            self.root_folder.find_mut(folder),
            Some(BookmarkNode::Folder { .. })
        ) {
            return Err("Destination folder no longer exists".into());
        }
        let items: Vec<_> = self
            .items()
            .into_iter()
            .filter(|b| ids.contains(&b.id))
            .collect();
        // Only remove leaf IDs; accidental folder IDs must not delete entire trees.
        let leaf_ids = items.iter().map(|b| b.id).collect();
        self.root_folder.remove(&leaf_ids);
        for bookmark in items {
            self.add(Some(folder), BookmarkNode::Bookmark(bookmark));
        }
        Ok(())
    }
}
