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
        Direction::Next => {
            let flat = (row * columns + column + 1).min(rows * columns - 1);
            (flat / columns, flat % columns)
        }
        Direction::Previous => {
            let flat = (row * columns + column).saturating_sub(1);
            (flat / columns, flat % columns)
        }
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
}
