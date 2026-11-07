use rustmarks::{
    merge::{MergeSession, Resolution},
    models::*,
};
fn document(title: &str, url: &str) -> BookmarkDocument {
    let mut doc = BookmarkDocument::default();
    doc.add(None, BookmarkNode::Bookmark(BookmarkItem::new(title, url)));
    doc
}
#[test]
fn conflicting_merges_require_explicit_decisions_without_touching_the_library() {
    let base = document("Existing", "https://example.com");
    let original = base.clone();
    let mut review = MergeSession::new(
        &base,
        vec![(
            "incoming.html".into(),
            document("Incoming", "https://example.com"),
        )],
    )
    .unwrap();
    assert_eq!(review.stats().unresolved, 1);
    assert!(!review.can_apply(&base));
    assert!(review.apply(&base).is_err());
    assert_eq!(base, original);
    review
        .set_decision(0, Some(Resolution::UseIncoming))
        .unwrap();
    let staged = review.apply(&base).unwrap();
    assert_eq!(staged.items()[0].title, "Incoming");
    assert_eq!(staged.items()[0].id, base.items()[0].id);
    assert_eq!(base, original);
    review.set_decision(0, None).unwrap();
    assert!(!review.can_apply(&base));
    assert!(review.apply(&base).is_err());
}
#[test]
fn a_changed_library_cannot_be_overwritten_by_a_stale_preview() {
    let base = document("Existing", "https://example.com");
    let mut review = MergeSession::new(
        &base,
        vec![(
            "incoming.json".into(),
            document("Incoming", "https://example.com"),
        )],
    )
    .unwrap();
    review.decide_all(Resolution::Combine);
    let mut current = base.clone();
    current
        .root_folder
        .visit_mut(&mut |bookmark| bookmark.notes = "Later edit".into());
    let before = current.clone();
    assert!(review.is_stale(&current));
    assert!(!review.can_apply(&current));
    assert!(review.apply(&current).is_err());
    assert_eq!(current, before);
}
#[test]
fn multi_file_counts_and_new_record_conflicts_are_accurate() {
    let base = document("Existing", "https://existing.test");
    let mut review = MergeSession::new(
        &base,
        vec![
            ("first.html".into(), document("First", "https://new.test")),
            (
                "second.html".into(),
                document("Second", "https://new.test?utm_source=mail"),
            ),
        ],
    )
    .unwrap();
    let stats = review.stats();
    assert_eq!(
        (
            stats.existing,
            stats.incoming,
            stats.total,
            stats.added,
            stats.duplicates,
            stats.conflicts,
            stats.unresolved
        ),
        (1, 2, 2, 1, 1, 1, 1)
    );
    assert_eq!(review.sources().len(), 2);
    review.decide_all(Resolution::UseIncoming);
    let staged = review.apply(&base).unwrap();
    assert_eq!(staged.items().len(), 2);
    assert!(staged
        .items()
        .iter()
        .any(|bookmark| bookmark.title == "Second"));
}
#[test]
fn no_conflict_merges_are_ready_and_empty_inputs_are_rejected() {
    let base = document("Existing", "https://example.com");
    assert!(MergeSession::new(&base, vec![]).is_err());
    let review = MergeSession::new(&base, vec![("same.json".into(), base.clone())]).unwrap();
    assert!(review.can_apply(&base));
    assert_eq!(review.stats().duplicates, 1);
    assert_eq!(review.apply(&base).unwrap(), base);
}
#[test]
fn invalid_decision_indices_and_invalid_documents_do_not_corrupt_the_review() {
    let base = document("Existing", "https://example.com");
    let mut review = MergeSession::new(
        &base,
        vec![(
            "incoming.html".into(),
            document("Incoming", "https://example.com"),
        )],
    )
    .unwrap();
    assert!(review
        .set_decision(10, Some(Resolution::UseIncoming))
        .is_err());
    assert_eq!(review.stats().unresolved, 1);
    let mut invalid = base.clone();
    let duplicate = invalid.items()[0].clone();
    invalid.add(None, BookmarkNode::Bookmark(duplicate));
    assert!(MergeSession::new(&base, vec![("invalid.json".into(), invalid)]).is_err());
}
#[test]
fn repeated_review_returns_the_same_candidate_without_mutating_the_session() {
    let mut base = document("Existing", "https://example.com");
    base.root_folder
        .visit_mut(&mut |bookmark| bookmark.notes = "Existing note".into());
    let mut incoming = document("Incoming", "https://example.com");
    incoming.root_folder.visit_mut(&mut |bookmark| {
        bookmark.notes = "Incoming note".into();
        bookmark.tags = vec!["dev".into()];
    });
    let mut review = MergeSession::new(&base, vec![("incoming.html".into(), incoming)]).unwrap();
    review.decide_all(Resolution::Combine);
    let first = review.apply(&base).unwrap();
    let second = review.apply(&base).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.items()[0].notes, "Existing note\nIncoming note");
    assert!(review.can_apply(&base));
}

#[test]
fn shipped_example_files_produce_reproducible_real_merge_statistics() {
    let base = rustmarks::parser::parse(include_str!("../examples/bookmarks.html")).unwrap();
    let incoming =
        rustmarks::parser::parse(include_str!("../examples/merge-incoming.html")).unwrap();
    let mut review = MergeSession::new(
        &base,
        vec![("examples/merge-incoming.html".into(), incoming)],
    )
    .unwrap();
    let stats = review.stats();
    assert_eq!(
        (
            stats.existing,
            stats.incoming,
            stats.total,
            stats.added,
            stats.duplicates,
            stats.conflicts,
            stats.unresolved
        ),
        (5, 4, 6, 1, 3, 2, 2)
    );
    review.decide_all(Resolution::Combine);
    assert_eq!(review.stats().unresolved, 0);
    assert_eq!(review.apply(&base).unwrap().items().len(), 6);
}
