//! Library health scan: fully local checks for the problems bookmark
//! libraries accumulate — duplicate URLs, empty folders, missing titles,
//! tracking parameters and malformed URLs. Scanning never mutates the
//! document; each fix is a separate, explicit operation the caller can
//! snapshot before applying.
use crate::models::{BookmarkDocument, BookmarkNode};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IssueKind {
    DuplicateUrl,
    EmptyFolder,
    MissingTitle,
    TrackingParameters,
    MalformedUrl,
}

impl IssueKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::DuplicateUrl => "Duplicate URLs",
            Self::EmptyFolder => "Empty folders",
            Self::MissingTitle => "Missing titles",
            Self::TrackingParameters => "Tracking parameters",
            Self::MalformedUrl => "Malformed URLs",
        }
    }
    /// All kinds, in report order.
    pub const ALL: [Self; 5] = [
        Self::DuplicateUrl,
        Self::EmptyFolder,
        Self::MissingTitle,
        Self::TrackingParameters,
        Self::MalformedUrl,
    ];
}

#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub kind: IssueKind,
    /// The affected bookmark, when the issue is about one.
    pub bookmark: Option<Uuid>,
    /// The affected folder (empty-folder issues) or the folder a bookmark
    /// sits in, for display context.
    pub folder: Option<Uuid>,
    /// One human-readable line describing the issue.
    pub detail: String,
    /// Whether `fix` can resolve it automatically.
    pub fixable: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HealthReport {
    pub issues: Vec<Issue>,
    pub bookmarks: usize,
    pub folders: usize,
}

impl HealthReport {
    pub fn count(&self, kind: IssueKind) -> usize {
        self.issues.iter().filter(|i| i.kind == kind).count()
    }
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }
}

struct FolderInfo {
    id: Uuid,
    path: String,
    bookmark_count: usize,
}

fn collect_folders(node: &BookmarkNode, path: &str, out: &mut Vec<FolderInfo>) {
    if let BookmarkNode::Folder {
        id,
        title,
        children,
        ..
    } = node
    {
        let here = if path.is_empty() {
            title.clone()
        } else {
            format!("{path} / {title}")
        };
        let mut count = 0;
        let mut stack: Vec<&BookmarkNode> = children.iter().collect();
        while let Some(node) = stack.pop() {
            match node {
                BookmarkNode::Bookmark(_) => count += 1,
                BookmarkNode::Folder { children, .. } => {
                    stack.extend(children.iter());
                }
            }
        }
        out.push(FolderInfo {
            id: *id,
            path: here.clone(),
            bookmark_count: count,
        });
        for child in children {
            collect_folders(child, &here, out);
        }
    }
}

fn collect_bookmarks(
    node: &BookmarkNode,
    folder: Option<Uuid>,
    folder_path: &str,
    out: &mut Vec<(Uuid, Option<Uuid>, String)>,
) {
    match node {
        BookmarkNode::Bookmark(b) => out.push((b.id, folder, folder_path.to_string())),
        BookmarkNode::Folder {
            id,
            title,
            children,
            ..
        } => {
            let here = if folder_path.is_empty() {
                title.clone()
            } else {
                format!("{folder_path} / {title}")
            };
            for child in children {
                collect_bookmarks(child, Some(*id), &here, out);
            }
        }
    }
}

/// Scan the whole document. Pure: the document is never changed.
pub fn scan(doc: &BookmarkDocument) -> HealthReport {
    let items = doc.items();
    let mut folders = Vec::new();
    collect_folders(&doc.root_folder, "", &mut folders);
    let mut locations = Vec::new();
    collect_bookmarks(&doc.root_folder, None, "", &mut locations);
    let folder_of: HashMap<Uuid, (Option<Uuid>, String)> = locations
        .into_iter()
        .map(|(id, folder, path)| (id, (folder, path)))
        .collect();

    let mut issues = Vec::new();
    // Duplicate URLs, keyed the same way the merge engine keys them.
    let mut by_url: HashMap<String, Vec<&crate::models::BookmarkItem>> = HashMap::new();
    for item in &items {
        by_url
            .entry(crate::merge::normalize(&item.url))
            .or_default()
            .push(item);
    }
    for group in by_url.values().filter(|g| g.len() > 1) {
        let first = group[0];
        for extra in &group[1..] {
            let (folder, path) = folder_of
                .get(&extra.id)
                .cloned()
                .unwrap_or((None, String::new()));
            issues.push(Issue {
                kind: IssueKind::DuplicateUrl,
                bookmark: Some(extra.id),
                folder,
                detail: format!(
                    "\u{201c}{}\u{201d} duplicates \u{201c}{}\u{201d} — {}",
                    extra.title, first.title, path
                ),
                fixable: true,
            });
        }
    }
    // Empty folders (no bookmarks anywhere below them), root excluded.
    let root_id = doc.root_folder.id();
    for folder in &folders {
        if folder.id != root_id && folder.bookmark_count == 0 {
            issues.push(Issue {
                kind: IssueKind::EmptyFolder,
                bookmark: None,
                folder: Some(folder.id),
                detail: format!("{} contains no bookmarks", folder.path),
                fixable: true,
            });
        }
    }
    // Per-bookmark checks.
    for item in &items {
        let (folder, _path) = folder_of
            .get(&item.id)
            .cloned()
            .unwrap_or((None, String::new()));
        if item.title.trim().is_empty() || item.title.trim() == item.url.trim() {
            issues.push(Issue {
                kind: IssueKind::MissingTitle,
                bookmark: Some(item.id),
                folder,
                detail: format!("{} has no title", item.url),
                fixable: derive_title(&item.url).is_some(),
            });
        }
        if crate::url_cleanup::clean(&item.url) != item.url {
            issues.push(Issue {
                kind: IssueKind::TrackingParameters,
                bookmark: Some(item.id),
                folder,
                detail: format!(
                    "\u{201c}{}\u{201d} — {} carries tracking parameters",
                    item.title, item.url
                ),
                fixable: true,
            });
        }
        let malformed = match url::Url::parse(&item.url) {
            Ok(parsed) => !matches!(parsed.scheme(), "http" | "https" | "ftp" | "file"),
            Err(_) => true,
        };
        if malformed {
            issues.push(Issue {
                kind: IssueKind::MalformedUrl,
                bookmark: Some(item.id),
                folder,
                detail: format!(
                    "\u{201c}{}\u{201d} — {} is not a usable URL",
                    item.title, item.url
                ),
                fixable: false,
            });
        }
    }
    HealthReport {
        issues,
        bookmarks: items.len(),
        folders: folders.len(),
    }
}

/// A readable title derived from a URL: the host, plus the last path
/// segment when it looks like words. `None` when nothing sensible exists.
pub fn derive_title(raw: &str) -> Option<String> {
    let parsed = url::Url::parse(raw).ok()?;
    let host = parsed.host_str()?.trim_start_matches("www.");
    let segment = parsed
        .path_segments()
        .and_then(|mut s| s.next_back())
        .unwrap_or("")
        .trim_end_matches(".html")
        .trim_end_matches(".htm")
        .replace(['-', '_'], " ");
    let segment = segment.trim();
    if segment.len() >= 3 && segment.chars().any(|c| c.is_alphabetic()) {
        let mut chars = segment.chars();
        let titled: String = match chars.next() {
            Some(first) => first.to_uppercase().chain(chars).collect(),
            None => segment.to_string(),
        };
        Some(format!("{titled} — {host}"))
    } else {
        Some(host.to_string())
    }
}

/// Apply the automatic fix for one issue. Returns true when the document
/// changed. Malformed URLs are never auto-fixed (a human should look).
pub fn fix(doc: &mut BookmarkDocument, issue: &Issue) -> bool {
    match issue.kind {
        IssueKind::DuplicateUrl => {
            let Some(id) = issue.bookmark else {
                return false;
            };
            let ids: std::collections::HashSet<Uuid> = [id].into_iter().collect();
            doc.root_folder.remove(&ids);
            true
        }
        IssueKind::EmptyFolder => {
            let Some(id) = issue.folder else { return false };
            let ids: std::collections::HashSet<Uuid> = [id].into_iter().collect();
            doc.root_folder.remove(&ids);
            true
        }
        IssueKind::TrackingParameters => {
            let Some(id) = issue.bookmark else {
                return false;
            };
            let mut changed = false;
            doc.root_folder.visit_mut(&mut |b| {
                if b.id == id {
                    b.url = crate::url_cleanup::clean(&b.url);
                    changed = true;
                }
            });
            changed
        }
        IssueKind::MissingTitle => {
            let Some(id) = issue.bookmark else {
                return false;
            };
            let mut changed = false;
            doc.root_folder.visit_mut(&mut |b| {
                if b.id == id {
                    if let Some(title) = derive_title(&b.url) {
                        b.title = title;
                        changed = true;
                    }
                }
            });
            changed
        }
        IssueKind::MalformedUrl => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::BookmarkItem;

    fn sample() -> BookmarkDocument {
        let mut doc = BookmarkDocument::default();
        let root = doc.root_folder.id();
        let a = BookmarkItem::new("Rust", "https://rust-lang.org/?utm_source=news");
        let b = BookmarkItem::new("Rust again", "https://rust-lang.org");
        let c = BookmarkItem::new("", "https://example.com/some-guide");
        let d = BookmarkItem::new("Broken", "https://");
        let empty = BookmarkNode::folder("Empty");
        doc.add(
            Some(root),
            BookmarkNode::Folder {
                id: Uuid::new_v4(),
                title: "Stuff".into(),
                children: vec![
                    BookmarkNode::Bookmark(a),
                    BookmarkNode::Bookmark(b),
                    BookmarkNode::Bookmark(c),
                    BookmarkNode::Bookmark(d),
                    empty,
                ],
                date_added: 0,
            },
        );
        doc
    }

    #[test]
    fn scan_finds_every_kind() {
        let report = scan(&sample());
        assert_eq!(report.bookmarks, 4);
        assert_eq!(report.count(IssueKind::DuplicateUrl), 1);
        assert_eq!(report.count(IssueKind::EmptyFolder), 1);
        assert_eq!(report.count(IssueKind::MissingTitle), 1);
        assert_eq!(report.count(IssueKind::TrackingParameters), 1);
        assert_eq!(report.count(IssueKind::MalformedUrl), 1);
        assert!(!report.is_clean());
    }

    #[test]
    fn fixes_resolve_their_issues() {
        let mut doc = sample();
        let report = scan(&doc);
        for issue in &report.issues {
            if issue.fixable {
                assert!(fix(&mut doc, issue), "fix should apply: {:?}", issue.kind);
            }
        }
        let after = scan(&doc);
        assert_eq!(after.count(IssueKind::DuplicateUrl), 0);
        assert_eq!(after.count(IssueKind::EmptyFolder), 0);
        assert_eq!(after.count(IssueKind::MissingTitle), 0);
        assert_eq!(after.count(IssueKind::TrackingParameters), 0);
        // The malformed placeholder stays — it needs a human.
        assert_eq!(after.count(IssueKind::MalformedUrl), 1);
        assert_eq!(after.bookmarks, 3);
    }


    #[test]
    fn derive_title_prefers_a_wordy_segment() {
        assert_eq!(
            derive_title("https://example.com/some-guide").as_deref(),
            Some("Some guide — example.com")
        );
        assert_eq!(
            derive_title("https://www.rust-lang.org/").as_deref(),
            Some("rust-lang.org")
        );
        assert_eq!(derive_title("not a url"), None);
    }
}
