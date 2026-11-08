use crate::models::{BookmarkDocument, BookmarkItem, BookmarkNode};
use uuid::Uuid;

/// The bookmarks the Library grid shows: everything under `folder`
/// (or the whole tree) whose title, URL, tags or notes match `query`.
/// Pure and allocation-light, so the GUI can cache its result and only
/// recompute when the document, folder or query actually changes.
pub fn visible_bookmarks(
    doc: &BookmarkDocument,
    folder: Option<Uuid>,
    query: &str,
) -> Vec<BookmarkItem> {
    fn find(node: &BookmarkNode, id: Uuid) -> Option<&BookmarkNode> {
        if node.id() == id {
            return Some(node);
        }
        if let BookmarkNode::Folder { children, .. } = node {
            for child in children {
                if let Some(found) = find(child, id) {
                    return Some(found);
                }
            }
        }
        None
    }
    let node = folder
        .and_then(|id| find(&doc.root_folder, id))
        .unwrap_or(&doc.root_folder);
    let mut items = vec![];
    node.visit(&mut |b| {
        if crate::heuristics::matches(b, query) {
            items.push(b.clone());
        }
    });
    items
}

/// Stable spreadsheet cell navigation independent of the GUI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Next,
    Previous,
}
pub fn navigate(
    row: usize,
    column: usize,
    rows: usize,
    columns: usize,
    direction: Direction,
) -> Option<(usize, usize)> {
    if rows == 0 || columns == 0 {
        return None;
    }
    let row = row.min(rows - 1);
    let column = column.min(columns - 1);
    Some(match direction {
        Direction::Up => (row.saturating_sub(1), column),
        Direction::Down => ((row + 1).min(rows - 1), column),
        // Coordinate arithmetic avoids overflowing rows * columns for large dimensions.
        Direction::Next if column < columns - 1 => (row, column + 1),
        Direction::Next if row < rows - 1 => (row + 1, 0),
        Direction::Next => (row, column),
        Direction::Previous if column > 0 => (row, column - 1),
        Direction::Previous if row > 0 => (row - 1, columns - 1),
        Direction::Previous => (row, column),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_tables_have_no_target() {
        assert_eq!(navigate(0, 0, 0, 4, Direction::Next), None);
        assert_eq!(navigate(0, 0, 4, 0, Direction::Down), None);
    }
    #[test]
    fn vertical_navigation_clamps_at_edges() {
        assert_eq!(navigate(0, 2, 3, 4, Direction::Up), Some((0, 2)));
        assert_eq!(navigate(2, 2, 3, 4, Direction::Down), Some((2, 2)));
        assert_eq!(navigate(1, 2, 3, 4, Direction::Up), Some((0, 2)));
    }
    #[test]
    fn tab_navigation_wraps_rows() {
        assert_eq!(navigate(0, 3, 3, 4, Direction::Next), Some((1, 0)));
        assert_eq!(navigate(1, 0, 3, 4, Direction::Previous), Some((0, 3)));
        assert_eq!(navigate(2, 3, 3, 4, Direction::Next), Some((2, 3)));
        assert_eq!(navigate(0, 0, 3, 4, Direction::Previous), Some((0, 0)));
    }
    #[test]
    fn stale_cell_coordinates_are_clamped_after_filtering() {
        assert_eq!(navigate(99, 99, 2, 4, Direction::Up), Some((0, 3)));
    }
    #[test]
    fn navigation_does_not_overflow_when_table_dimensions_are_large() {
        let size = usize::MAX;
        assert_eq!(
            navigate(size, size, size, size, Direction::Next),
            Some((size - 1, size - 1))
        );
        assert_eq!(
            navigate(size - 2, size - 1, size, size, Direction::Next),
            Some((size - 1, 0))
        );
        assert_eq!(
            navigate(size - 1, 0, size, size, Direction::Previous),
            Some((size - 2, size - 1))
        );
    }
    #[test]
    fn cell_navigation_matches_small_table_reference_for_every_cell() {
        for rows in 1usize..=5 {
            for columns in 1..=5 {
                for row in 0..rows {
                    for column in 0..columns {
                        let flat = row * columns + column;
                        let next = (flat + 1).min(rows * columns - 1);
                        let previous = flat.saturating_sub(1);
                        assert_eq!(
                            navigate(row, column, rows, columns, Direction::Next),
                            Some((next / columns, next % columns))
                        );
                        assert_eq!(
                            navigate(row, column, rows, columns, Direction::Previous),
                            Some((previous / columns, previous % columns))
                        );
                    }
                }
            }
        }
    }

    /// Scale guard: filtering a 10,000-bookmark library stays fast and
    /// correct, so typing in the search box never stalls the window.
    #[test]
    fn ten_thousand_bookmarks_filter_quickly() {
        use crate::models::BookmarkItem;
        let mut doc = BookmarkDocument::default();
        let root = doc.root_folder.id();
        for f in 0..100 {
            let mut children = Vec::new();
            for i in 0..100 {
                children.push(BookmarkNode::Bookmark(BookmarkItem::new(
                    format!("Site {f}-{i}"),
                    format!("https://example{f}.com/page-{i}"),
                )));
            }
            doc.add(
                Some(root),
                BookmarkNode::Folder {
                    id: Uuid::new_v4(),
                    title: format!("Folder {f}"),
                    children,
                    date_added: 0,
                },
            );
        }
        assert_eq!(doc.items().len(), 10_000);
        let started = std::time::Instant::now();
        let all = visible_bookmarks(&doc, None, "");
        let some = visible_bookmarks(&doc, None, "example42");
        let elapsed = started.elapsed();
        assert_eq!(all.len(), 10_000);
        assert_eq!(some.len(), 100);
        assert!(
            elapsed < std::time::Duration::from_secs(5),
            "filtering 10k bookmarks took {elapsed:?}"
        );
    }
}
