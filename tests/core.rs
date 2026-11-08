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
        "https://example.com/a?b=2&a=1#part"
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

#[test]
fn history_prune_keeps_the_newest_snapshots() {
    let directory = std::env::temp_dir().join(format!("rustmarks-test-{}", uuid::Uuid::new_v4()));
    let h = History {
        directory: directory.clone(),
    };
    let d = document(&[("Test", "https://example.com")]);
    for _ in 0..5 {
        h.save(&d, "test").unwrap();
    }
    assert_eq!(h.list().unwrap().len(), 5);
    assert!(h.total_size() > 0);
    assert_eq!(h.prune(0).unwrap(), 0, "0 keeps everything");
    assert_eq!(h.prune(2).unwrap(), 3);
    assert_eq!(h.list().unwrap().len(), 2);
    assert_eq!(
        h.prune(10).unwrap(),
        0,
        "keeping more than exist removes none"
    );
    // The survivors still restore.
    let snapshots = h.list().unwrap();
    assert_eq!(h.restore(&snapshots[0]).unwrap(), d);
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn imports_bom_and_legacy_descriptions() {
    let html="\u{feff}<DL><p><DT><A HREF='https://example.com'>Example</A><DD>A &amp; B <b>description</b>\n<DT><A HREF='https://two.test'>Two</A></DL>";
    let doc = parser::parse(html).unwrap();
    assert_eq!(doc.items().len(), 2);
    assert_eq!(doc.items()[0].notes, "A & B description");
    let native = format!("\u{feff}{}", serde_json::to_string(&doc).unwrap());
    assert_eq!(parser::parse(&native).unwrap(), doc);
}
#[test]
fn corrupt_native_documents_are_rejected() {
    let mut doc = document(&[("Example", "https://example.com")]);
    let duplicate = doc.items()[0].clone();
    doc.add(None, BookmarkNode::Bookmark(duplicate));
    assert!(parser::parse(&serde_json::to_string(&doc).unwrap()).is_err());
    doc.version = 99;
    assert!(doc.validate().is_err());
    doc.version = 1;
    doc.root_folder =
        BookmarkNode::Bookmark(BookmarkItem::new("Invalid root", "https://example.com"));
    assert!(doc.validate().is_err());
}
#[test]
fn import_depth_limits_reject_hostile_html() {
    let mut html = "<DL>".to_owned();
    for _ in 0..130 {
        html.push_str("<DT><H3>Nested</H3><DL>");
    }
    html.push_str("<DT><A HREF='https://example.com'>Test</A>");
    assert!(parser::parse(&html).is_err());
}
#[test]
fn user_rules_validate_and_break_ties_deterministically() {
    use rustmarks::rules::RuleSet;
    let rules=RuleSet::parse(r#"{"version":1,"rules":[{"category":"First","domains":["EXAMPLE.COM"],"keywords":[]},{"category":"Second","domains":["example.com"],"keywords":[] }]}"#).unwrap();
    assert_eq!(
        rules.classify(&BookmarkItem::new("Example", "https://sub.example.com")),
        Some("First".into())
    );
    assert_eq!(
        rules.classify(&BookmarkItem::new("Example", "https://notexample.com")),
        None
    );
    assert!(RuleSet::parse(
        r#"{"version":1,"rules":[{"category":"Bad","domains":["https://example.com"]}]}"#
    )
    .is_err());
    assert!(RuleSet::parse(r#"{"version":2,"rules":[]}"#).is_err());
    let encoded = serde_json::to_string(&RuleSet::default()).unwrap();
    assert_eq!(RuleSet::parse(&encoded).unwrap(), RuleSet::default());
}
#[test]
fn session_undo_redo_and_branching() {
    use rustmarks::undo::UndoHistory;
    let initial = document(&[("One", "https://one.test")]);
    let mut doc = initial.clone();
    let mut undo = UndoHistory::default();
    undo.record(doc.clone(), false);
    doc.add(
        None,
        BookmarkNode::Bookmark(BookmarkItem::new("Two", "https://two.test")),
    );
    let edited = doc.clone();
    assert!(undo.undo(&mut doc));
    assert_eq!(doc, initial);
    assert!(undo.redo(&mut doc));
    assert_eq!(doc, edited);
    assert!(undo.undo(&mut doc));
    undo.record(doc.clone(), false);
    doc.add(
        None,
        BookmarkNode::Bookmark(BookmarkItem::new("Three", "https://three.test")),
    );
    assert!(!undo.can_redo());
}
#[test]
fn undo_coalesces_typing_and_bounds_history() {
    use rustmarks::undo::UndoHistory;
    let mut doc = document(&[("One", "https://one.test")]);
    let initial = doc.clone();
    let mut undo = UndoHistory::default();
    for i in 0..3 {
        undo.record(doc.clone(), i > 0);
        doc.root_folder
            .visit_mut(&mut |b| b.title = format!("Title {i}"));
    }
    assert!(undo.undo(&mut doc));
    assert_eq!(doc, initial);
    assert!(!undo.can_undo());
    for i in 0..75 {
        undo.record(doc.clone(), false);
        doc.root_folder.visit_mut(&mut |b| b.notes = i.to_string());
    }
    let mut count = 0;
    while undo.undo(&mut doc) {
        count += 1;
    }
    assert_eq!(count, 50);
}
#[test]
fn diff_catches_moves_between_same_named_folders_and_reordering() {
    let mut base = BookmarkDocument::default();
    let mut first = BookmarkNode::folder("Same");
    let second = BookmarkNode::folder("Same");
    let bookmark = BookmarkItem::new("Move me", "https://example.com");
    let id = bookmark.id;
    first
        .children_mut()
        .unwrap()
        .push(BookmarkNode::Bookmark(bookmark.clone()));
    let destination = second.id();
    base.add(None, first);
    base.add(None, second);
    let mut edited = base.clone();
    edited.root_folder.remove(&[id].into_iter().collect());
    edited.add(Some(destination), BookmarkNode::Bookmark(bookmark));
    assert!(diff_view::changes(&base, &edited)
        .iter()
        .any(|c| c.id == id && c.kind == Kind::Moved));
    let base = document(&[("One", "https://one.test"), ("Two", "https://two.test")]);
    let mut edited = base.clone();
    edited.root_folder.children_mut().unwrap().reverse();
    assert_eq!(diff_view::changes(&base, &edited).len(), 2);
    let mut inserted = base.clone();
    inserted.root_folder.children_mut().unwrap().insert(
        0,
        BookmarkNode::Bookmark(BookmarkItem::new("New", "https://new.test")),
    );
    assert_eq!(diff_view::changes(&base, &inserted).len(), 1);
}
#[test]
fn favicon_normalization_is_bounded_and_roundtrips() {
    use rustmarks::favicon;
    let image = image::RgbaImage::from_pixel(64, 48, image::Rgba([0, 122, 204, 255]));
    let mut buffer = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut buffer, image::ImageFormat::Png)
        .unwrap();
    let uri = favicon::from_bytes(buffer.get_ref()).unwrap();
    let (w, h, pixels) = favicon::pixels(&uri).unwrap();
    assert_eq!((w, h), (32, 24));
    assert_eq!(pixels.len(), w * h * 4);
    assert!(favicon::from_bytes(&vec![0; 2 * 1024 * 1024 + 1]).is_err());
    assert!(favicon::pixels("https://example.com/favicon.ico").is_err());
    assert!(favicon::pixels("data:image/png;base64,bad!").is_err());
}
#[test]
fn atomic_write_failure_does_not_remove_existing_target() {
    let directory = std::env::temp_dir().join(format!("rustmarks-atomic-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let target = directory.join("destination");
    std::fs::create_dir(&target).unwrap();
    let child = target.join("precious.txt");
    std::fs::write(&child, b"keep me").unwrap();
    assert!(rustmarks::storage::atomic_write(&target, b"replacement").is_err());
    assert_eq!(std::fs::read(&child).unwrap(), b"keep me");
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn duplicate_records_inside_one_import_resolve_against_new_identity() {
    let base = BookmarkDocument::default();
    let incoming = document(&[
        ("First", "https://example.com"),
        ("Second", "https://example.com?utm_source=test"),
    ]);
    let mut preview = merge::preview(&base, &incoming);
    assert_eq!(preview.added, 1);
    assert_eq!(preview.conflicts.len(), 1);
    merge::resolve(
        &mut preview.document,
        &preview.conflicts[0],
        Resolution::UseIncoming,
    );
    assert_eq!(preview.document.items()[0].title, "Second");
}

#[test]
fn moving_bookmarks_preserves_identity_and_rejects_invalid_destinations() {
    let mut doc = document(&[("One", "https://one.test"), ("Two", "https://two.test")]);
    let original = doc.items();
    let folder = BookmarkNode::folder("Destination");
    let folder_id = folder.id();
    doc.add(None, folder);
    let selection = [original[0].id].into_iter().collect();
    doc.move_bookmarks(&selection, folder_id).unwrap();
    assert_eq!(doc.items().len(), 2);
    assert!(doc.items().iter().any(|b| b.id == original[0].id));
    let before = doc.clone();
    assert!(doc
        .move_bookmarks(&selection, uuid::Uuid::new_v4())
        .is_err());
    assert_eq!(doc, before);
    // Including a folder ID in a bulk selection must not delete the folder.
    doc.move_bookmarks(&[folder_id].into_iter().collect(), doc.root_folder.id())
        .unwrap();
    assert!(doc.folders().iter().any(|(id, _)| *id == folder_id));
}
#[test]
fn chrome_export_handles_extreme_native_timestamps_without_overflow() {
    let mut doc = document(&[("Future", "https://example.com")]);
    doc.root_folder.visit_mut(&mut |b| b.date_added = i64::MAX);
    let exported = parser::chrome(&doc);
    let imported = parser::parse(&exported.to_string()).unwrap();
    assert_eq!(imported.items()[0].date_added, i64::MAX);
}

#[test]
fn favicon_only_changes_have_distinct_review_summaries() {
    let mut before = document(&[("Icon", "https://example.com")]);
    before
        .root_folder
        .visit_mut(&mut |b| b.favicon_base64 = Some("data:image/png;base64,AA==".into()));
    let mut after = before.clone();
    after
        .root_folder
        .visit_mut(&mut |b| b.favicon_base64 = Some("data:image/png;base64,BB==".into()));
    let changes = diff_view::changes(&before, &after);
    assert_eq!(changes.len(), 1);
    assert_ne!(changes[0].before, changes[0].after);
}

#[test]
fn tracking_cleanup_and_deduplication_preserve_distinct_fragment_routes() {
    let base = document(&[("Route A", "https://example.com/#/a")]);
    let incoming = document(&[("Route B", "https://example.com/#/b")]);
    let preview = merge::preview(&base, &incoming);
    assert_eq!(preview.added, 1);
    assert_eq!(preview.duplicates, 0);
    let url = "https://EXAMPLE.com:443/?b=%20&utm_source=mail&a=1&a=2#/route";
    assert_eq!(
        rustmarks::url_cleanup::clean(url),
        "https://EXAMPLE.com:443/?b=%20&a=1&a=2#/route"
    );
}
#[test]
fn repeated_merge_combine_does_not_repeat_notes_or_tags() {
    let mut base = document(&[("Original", "https://example.com")]);
    base.root_folder
        .visit_mut(&mut |b| b.notes = "Original notes".into());
    let mut incoming = document(&[("Incoming", "https://example.com")]);
    incoming.root_folder.visit_mut(&mut |b| {
        b.notes = "First line\nSecond line".into();
        b.tags = vec!["dev".into()];
    });
    for _ in 0..3 {
        let preview = merge::preview(&base, &incoming);
        base = preview.document;
        for conflict in preview.conflicts {
            merge::resolve(&mut base, &conflict, Resolution::Combine);
        }
    }
    assert_eq!(
        base.items()[0].notes,
        "Original notes\nFirst line\nSecond line"
    );
    assert_eq!(base.items()[0].tags, vec!["dev"]);
}
#[test]
fn merging_preserves_intentionally_empty_folders_but_omits_duplicate_only_wrappers() {
    let base = document(&[("Existing", "https://example.com")]);
    let mut incoming = document(&[("Existing", "https://example.com")]);
    incoming.add(None, BookmarkNode::folder("Empty folder"));
    let preview = merge::preview(&base, &incoming);
    assert_eq!(preview.added, 0);
    assert!(preview
        .document
        .folders()
        .iter()
        .any(|(_, path)| path.ends_with("/Empty folder")));
    let identical = document(&[("Existing", "https://example.com")]);
    let preview = merge::preview(&base, &identical);
    assert_eq!(preview.document, base);
}
#[test]
fn tracked_save_and_load_capture_the_published_document_format() {
    let directory =
        std::env::temp_dir().join(format!("rustmarks-tracked-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let document = document(&[("Example", "https://example.com")]);
    let path = directory.join("bookmarks.json");
    let revision = parser::save_tracked(&document, &path).unwrap();
    revision.ensure_unchanged().unwrap();
    let loaded = parser::load_tracked(&path).unwrap();
    assert_eq!(loaded.format, parser::SourceFormat::NativeJson);
    assert_eq!(loaded.document.items(), document.items());
    let browser = directory.join("chrome.json");
    std::fs::write(&browser, parser::chrome(&document).to_string()).unwrap();
    assert_eq!(
        parser::load_tracked(&browser).unwrap().format,
        parser::SourceFormat::ChromeJson
    );
    let html = directory.join("bookmarks.html");
    parser::save(&document, &html).unwrap();
    assert_eq!(
        parser::load_tracked(&html).unwrap().format,
        parser::SourceFormat::NetscapeHtml
    );
    std::fs::remove_dir_all(&directory).unwrap();
}
#[cfg(unix)]
#[test]
fn saving_through_a_symlink_updates_target_without_replacing_link() {
    let directory =
        std::env::temp_dir().join(format!("rustmarks-symlink-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let target = directory.join("target.json");
    let alias = directory.join("alias.json");
    let mut doc = document(&[("Before", "https://example.com")]);
    parser::save(&doc, &target).unwrap();
    std::os::unix::fs::symlink(&target, &alias).unwrap();
    doc.root_folder.visit_mut(&mut |b| b.title = "After".into());
    let revision = parser::save_tracked(&doc, &alias).unwrap();
    assert!(std::fs::symlink_metadata(&alias)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(parser::load(&target).unwrap().items()[0].title, "After");
    revision.ensure_unchanged().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn checked_save_refuses_external_changes_and_allows_a_separate_working_copy() {
    let directory =
        std::env::temp_dir().join(format!("rustmarks-conflict-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("source.json");
    let doc = document(&[("Original", "https://example.com")]);
    parser::save(&doc, &path).unwrap();
    let loaded = parser::load_tracked(&path).unwrap();
    std::fs::write(&path, b"external changes").unwrap();
    assert!(parser::save_checked(&doc, &path, Some(&loaded.revision)).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"external changes");
    let other = directory.join("working.json");
    parser::save_checked(&doc, &other, Some(&loaded.revision)).unwrap();
    assert_eq!(parser::load(&other).unwrap().items(), doc.items());
    std::fs::remove_file(&path).unwrap();
    assert!(parser::save_checked(&doc, &path, Some(&loaded.revision)).is_err());
    assert!(!path.exists());
    std::fs::remove_dir_all(directory).unwrap();
}

mod window_state_tests {
    use rustmarks::window_state::*;
    fn dir() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("rm-ws-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    #[test]
    fn missing_or_corrupt_files_give_defaults() {
        let d = dir();
        assert_eq!(
            WindowState::load(&d.join("none.json")),
            WindowState::default()
        );
        let p = d.join("bad.json");
        std::fs::write(&p, b"{not json").unwrap();
        assert_eq!(WindowState::load(&p), WindowState::default());
        std::fs::write(&p, vec![b' '; 10_000]).unwrap();
        assert_eq!(WindowState::load(&p), WindowState::default());
    }
    #[test]
    fn roundtrip_persists_size_position_and_maximized() {
        let d = dir();
        let p = d.join("sub/window.json");
        let s = WindowState {
            size: [1200., 800.],
            position: Some([-300., 40.]),
            maximized: true,
        };
        s.save(&p).unwrap();
        assert_eq!(WindowState::load(&p), s);
    }
    #[test]
    fn invalid_values_are_clamped_or_dropped() {
        let s = WindowState {
            size: [10., 99999.],
            position: Some([1.0e9, 0.]),
            maximized: false,
        }
        .sanitized();
        assert_eq!(s.size, [MIN_SIZE[0], 16384.]);
        assert_eq!(s.position, None);
        let nan = WindowState {
            size: [f32::NAN, 800.],
            position: Some([f32::INFINITY, 0.]),
            maximized: false,
        }
        .sanitized();
        assert_eq!(nan.size, DEFAULT_SIZE);
        assert_eq!(nan.position, None);
    }
}

mod paths_tests {
    use rustmarks::paths::*;
    #[test]
    fn marker_selects_adjacent_data_dir_else_user_dir() {
        let d = std::env::temp_dir().join(format!("rm-paths-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        let user = std::path::PathBuf::from("/u");
        assert_eq!(resolve(Some(&d), user.clone()), user.join("RustMarks"));
        assert_eq!(resolve(None, user.clone()), user.join("RustMarks"));
        std::fs::write(d.join(PORTABLE_MARKER), b"").unwrap();
        assert_eq!(resolve(Some(&d), user), d.join("data"));
    }
}
