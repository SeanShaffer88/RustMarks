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
}
