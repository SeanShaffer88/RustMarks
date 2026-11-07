//! Dependency-free, explicit conflict decisions shared by the engine and native UI.
use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution {
    KeepExisting,
    UseIncoming,
    Combine,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReviewError {
    Stale,
    Unresolved(usize),
    MissingConflict { index: usize, total: usize },
}
impl fmt::Display for ReviewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stale=>write!(f,"Your library changed after the preview. Refresh the incoming files before applying."),
            Self::Unresolved(count)=>write!(f,"Choose a resolution for all {count} remaining metadata conflicts before reviewing changes."),
            Self::MissingConflict{index,total}=>write!(f,"Conflict {index} does not exist in a review with {total} conflicts"),
        }
    }
}
impl std::error::Error for ReviewError {}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewState {
    choices: Vec<Option<Resolution>>,
}
impl ReviewState {
    pub fn new(count: usize) -> Self {
        Self {
            choices: vec![None; count],
        }
    }
    pub fn get(&self, index: usize) -> Option<Resolution> {
        self.choices.get(index).copied().flatten()
    }
    pub fn set(&mut self, index: usize, choice: Option<Resolution>) -> Result<(), ReviewError> {
        let total = self.choices.len();
        let entry = self
            .choices
            .get_mut(index)
            .ok_or(ReviewError::MissingConflict { index, total })?;
        *entry = choice;
        Ok(())
    }
    pub fn set_all(&mut self, choice: Resolution) {
        self.choices.fill(Some(choice));
    }
    pub fn remaining(&self) -> usize {
        self.choices.iter().filter(|entry| entry.is_none()).count()
    }
    pub fn iter(&self) -> impl Iterator<Item = Option<Resolution>> + '_ {
        self.choices.iter().copied()
    }
    pub fn validate(&self, stale: bool) -> Result<(), ReviewError> {
        if stale {
            return Err(ReviewError::Stale);
        }
        let remaining = self.remaining();
        if remaining > 0 {
            return Err(ReviewError::Unresolved(remaining));
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_new_review_never_silently_defaults_to_a_resolution() {
        let review = ReviewState::new(2);
        assert_eq!(review.remaining(), 2);
        assert_eq!(review.get(0), None);
        assert_eq!(review.validate(false), Err(ReviewError::Unresolved(2)));
    }
    #[test]
    fn every_conflict_needs_a_choice_and_a_choice_can_be_cleared() {
        let mut review = ReviewState::new(2);
        review.set(0, Some(Resolution::KeepExisting)).unwrap();
        assert_eq!(review.validate(false), Err(ReviewError::Unresolved(1)));
        review.set(1, Some(Resolution::UseIncoming)).unwrap();
        assert_eq!(review.validate(false), Ok(()));
        review.set(0, None).unwrap();
        assert_eq!(review.remaining(), 1);
        assert!(review.validate(false).is_err());
    }
    #[test]
    fn explicitly_applying_to_all_is_ready_and_preserves_ordered_indices() {
        let mut review = ReviewState::new(3);
        review.set_all(Resolution::Combine);
        assert_eq!(
            review.iter().collect::<Vec<_>>(),
            vec![Some(Resolution::Combine); 3]
        );
        assert_eq!(review.validate(false), Ok(()));
    }
    #[test]
    fn a_stale_review_is_blocked_even_when_all_decisions_exist() {
        let mut review = ReviewState::new(2);
        review.set_all(Resolution::Combine);
        assert_eq!(review.validate(true), Err(ReviewError::Stale));
    }
    #[test]
    fn conflict_free_reviews_are_ready_only_when_fresh() {
        let review = ReviewState::new(0);
        assert_eq!(review.validate(false), Ok(()));
        assert_eq!(review.validate(true), Err(ReviewError::Stale));
    }
    #[test]
    fn invalid_indices_do_not_change_any_decisions() {
        let mut review = ReviewState::new(2);
        review.set(0, Some(Resolution::KeepExisting)).unwrap();
        let before = review.clone();
        assert_eq!(
            review.set(2, Some(Resolution::UseIncoming)),
            Err(ReviewError::MissingConflict { index: 2, total: 2 })
        );
        assert_eq!(review, before);
    }
    #[test]
    fn a_filtered_row_index_does_not_resolve_hidden_conflicts() {
        let mut review = ReviewState::new(3);
        review.set(1, Some(Resolution::UseIncoming)).unwrap();
        assert_eq!(review.get(0), None);
        assert_eq!(review.get(1), Some(Resolution::UseIncoming));
        assert_eq!(review.get(2), None);
        assert_eq!(review.validate(false), Err(ReviewError::Unresolved(2)));
    }
}
