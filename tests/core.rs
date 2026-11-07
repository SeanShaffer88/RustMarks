use rustmarks::{
    diff_view::{self, Kind},
    heuristics,
    history::History,
    merge::{self, Resolution},
    models::*,
    parser,
};
fn document(bookmarks: &[(&str, &str)]) -> BookmarkDocument {
    let mut d = BookmarkDocument::default();
    for (title, url) in bookmarks {
        d.add(
            None,
            BookmarkNode::Bookmark(BookmarkItem::new(*title, *url)),
        );
    }
    d
}
#[test]
fn native_json_preserves_all_metadata_and_identity() {
    let mut d = document(&[("Rust", "https://rust-lang.org")]);
    d.root_folder.visit_mut(&mut |b| {
        b.notes = "A note\nwith lines".into();
        b.tags = vec!["dev".into()];
        b.favicon_base64 = Some("data:image/png;base64,aA==".into());
    });
    assert_eq!(
        parser::parse(&serde_json::to_string(&d).unwrap()).unwrap(),
        d
    );
}
#[test]
fn html_roundtrip_nested_folders_escaping_metadata() {
    let mut d = BookmarkDocument::default();
    let mut folder = BookmarkNode::folder("Dev & <tools>");
    let mut b = BookmarkItem::new("Rust & \"friends\"", "https://example.com/?a=1&b=2");
    b.tags = vec!["dev".into(), "rust".into()];
    b.notes = "<note> & \"quote\"".into();
    b.favicon_base64 = Some("data:image/png;base64,AA==".into());
    folder
        .children_mut()
        .unwrap()
        .push(BookmarkNode::Bookmark(b.clone()));
    let folder_id = folder.id();
    d.add(None, folder);
    let parsed = parser::parse(&parser::html(&d)).unwrap();
    let actual = parsed.items().remove(0);
    assert_eq!(actual.title, b.title);
    assert_eq!(actual.url, b.url);
    assert_eq!(actual.tags, b.tags);
    assert_eq!(actual.notes, b.notes);
    assert_eq!(actual.favicon_base64, b.favicon_base64);
    assert!(parser::html(&parsed).contains("Dev &amp; &lt;tools&gt;"));
    assert_ne!(parsed.root_folder.id(), folder_id);
}
#[test]
fn html_sibling_folders_remain_separate() {
    let html="<DL><p><DT><H3>One</H3><DL><p><DT><A HREF='https://one.test'>One</A></DL><p><DT><H3>Two</H3><DL><p><DT><A HREF='https://two.test'>Two</A></DL><p></DL>";
    let d = parser::parse(html).unwrap();
    assert_eq!(d.items().len(), 2);
    if let BookmarkNode::Folder { children, .. } = &d.root_folder {
        assert_eq!(children.len(), 2);
        assert_eq!(children[1].title(), "Two");
    } else {
        panic!("missing root")
    }
}
#[test]
fn chrome_roundtrip_and_epoch_conversion() {
    let d = document(&[("Example", "https://example.com")]);
    let p = parser::parse(&parser::chrome(&d).to_string()).unwrap();
    assert_eq!(p.items()[0].date_added, d.items()[0].date_added);
    assert_eq!(p.items()[0].url, "https://example.com");
    assert!(parser::parse("{\"unrelated\":true}").is_err());
    assert!(parser::parse("not bookmarks").is_err());
}
#[test]
fn heuristics_use_domain_boundaries_and_support_filter_expressions() {
    assert_eq!(
        heuristics::classify(&BookmarkItem::new(
            "Repository",
            "https://github.com/example"
        )),
        Some("Developer & Tech".into())
    );
    assert_eq!(
        heuristics::classify(&BookmarkItem::new("Example", "https://notgithub.com/")),
        None
    );
    let mut b = BookmarkItem::new("Rust docs", "https://docs.rs");
    b.tags = vec!["dev".into()];
    assert!(heuristics::matches(&b, "tag:dev rust -broken"));
    assert!(!heuristics::matches(&b, "tag:design"));
    assert!(!heuristics::matches(&b, "-rust"));
}
#[test]
fn normalization_keeps_meaningful_query_parameters() {
    assert_eq!(
        merge::normalize("https://EXAMPLE.com:443/a?utm_source=x&b=2&a=1#part"),
        "https://example.com/a?a=1&b=2"
    );
    assert_ne!(
        merge::normalize("https://example.com/?id=1"),
        merge::normalize("https://example.com/?id=2")
    );
}
#[test]
fn merge_conflicts_are_staged_and_preserve_existing_identity() {
    let mut base = document(&[("Old title", "https://example.com/?id=1")]);
    base.root_folder.visit_mut(&mut |b| {
        b.tags = vec!["old".into()];
        b.notes = "old note".into();
    });
    let mut incoming = document(&[
        ("New title", "https://example.com/?utm_source=mail&id=1"),
        ("New", "https://new.test"),
    ]);
    incoming.root_folder.visit_mut(&mut |b| {
        b.tags = vec!["new".into()];
        b.notes = "new note".into();
    });
    let mut p = merge::preview(&base, &incoming);
    assert_eq!(p.added, 1);
    assert_eq!(p.duplicates, 1);
    assert_eq!(p.conflicts.len(), 1);
    assert_eq!(base.items().len(), 1);
    merge::resolve(&mut p.document, &p.conflicts[0], Resolution::Combine);
    let item = p
        .document
        .items()
        .into_iter()
        .find(|b| b.id == base.items()[0].id)
        .unwrap();
    assert_eq!(item.title, "Old title");
    assert_eq!(item.tags, vec!["old", "new"]);
    assert_eq!(item.notes, "old note\nnew note");
    merge::resolve(&mut p.document, &p.conflicts[0], Resolution::UseIncoming);
    assert_eq!(
        p.document
            .items()
            .into_iter()
            .find(|b| b.id == base.items()[0].id)
            .unwrap()
            .title,
        "New title"
    );
}
#[test]
fn organizer_moves_instead_of_duplicating() {
    let mut d = document(&[
        ("Rust", "https://docs.rs"),
        ("Unknown", "https://example.com"),
    ]);
    let id = d.items()[0].id;
    heuristics::suggest(&mut d);
    heuristics::organize(&mut d);
    assert_eq!(d.items().len(), 2);
    assert!(d.items().iter().any(|b| b.id == id));
    let original = d.clone();
    heuristics::organize(&mut d);
    assert_eq!(original, d);
}
#[test]
fn diff_detects_metadata_additions_deletions_and_moves() {
    let base = document(&[("One", "https://one.test"), ("Two", "https://two.test")]);
    let mut edited = base.clone();
    let first = base.items()[0].id;
    let second = base.items()[1].id;
    edited.root_folder.visit_mut(&mut |b| {
        if b.id == first {
            b.notes = "changed".into();
        }
    });
    let mut ids = std::collections::HashSet::new();
    ids.insert(second);
    edited.root_folder.remove(&ids);
    edited.add(
        None,
        BookmarkNode::Bookmark(BookmarkItem::new("Three", "https://three.test")),
    );
    let changes = diff_view::changes(&base, &edited);
    assert_eq!(changes.len(), 3);
    assert!(changes.iter().any(|c| c.kind == Kind::Modified));
    assert!(changes.iter().any(|c| c.kind == Kind::Removed));
    assert!(changes.iter().any(|c| c.kind == Kind::Added));
    let mut moved = base.clone();
    let b = moved.items()[0].clone();
    let mut ids = std::collections::HashSet::new();
    ids.insert(b.id);
    moved.root_folder.remove(&ids);
    let mut f = BookmarkNode::folder("Moved");
    f.children_mut().unwrap().push(BookmarkNode::Bookmark(b));
    moved.add(None, f);
    assert!(diff_view::changes(&base, &moved)
        .iter()
        .any(|c| c.kind == Kind::Moved));
}
#[test]
fn compressed_history_roundtrip_and_disk_save() {
    let directory = std::env::temp_dir().join(format!("rustmarks-test-{}", uuid::Uuid::new_v4()));
    let h = History {
        directory: directory.clone(),
    };
    let d = document(&[("Test", "https://example.com")]);
    h.save(&d, "test").unwrap();
    let snapshots = h.list().unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(h.restore(&snapshots[0]).unwrap(), d);
    let file = directory.join("bookmarks.json");
    parser::save(&d, &file).unwrap();
    parser::save(&d, &file).unwrap();
    assert_eq!(parser::load(&file).unwrap().items()[0].title, "Test");
    std::fs::remove_dir_all(directory).unwrap();
}
