use crate::models::BookmarkDocument;
use std::collections::VecDeque;
/// Bounded session undo. Disk snapshots remain the durable history.
pub struct UndoHistory {
    undo: VecDeque<BookmarkDocument>,
    redo: Vec<BookmarkDocument>,
    limit: usize,
}
impl Default for UndoHistory {
    fn default() -> Self {
        Self {
            undo: VecDeque::new(),
            redo: vec![],
            limit: 50,
        }
    }
}
impl UndoHistory {
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn record(&mut self, before: BookmarkDocument, coalesce: bool) {
        if !coalesce || self.undo.is_empty() {
            self.undo.push_back(before);
        }
        while self.undo.len() > self.limit {
            self.undo.pop_front();
        }
        self.redo.clear();
    }
    pub fn undo(&mut self, current: &mut BookmarkDocument) -> bool {
        if let Some(previous) = self.undo.pop_back() {
            self.redo.push(std::mem::replace(current, previous));
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self, current: &mut BookmarkDocument) -> bool {
        if let Some(next) = self.redo.pop() {
            self.undo.push_back(std::mem::replace(current, next));
            true
        } else {
            false
        }
    }
}
