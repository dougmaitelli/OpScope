//! Summaries derived from Bitbucket's individual build statuses and participants.

use crate::domain::{ChangeRequestCheckStatus as Check, ChangeRequestReviewStatus as Review};

pub(super) fn checks(states: impl IntoIterator<Item = Check>) -> Check {
    let states: Vec<_> = states.into_iter().collect();
    if states.is_empty() {
        Check::None
    } else if states.contains(&Check::Failing) {
        Check::Failing
    } else if states.contains(&Check::Running) {
        Check::Running
    } else if !states.is_empty() && states.iter().all(|state| *state == Check::Passed) {
        Check::Passed
    } else {
        Check::Unknown
    }
}

pub(super) fn reviews(states: impl IntoIterator<Item = Review>) -> Review {
    let states: Vec<_> = states.into_iter().collect();
    if states.is_empty() {
        Review::None
    } else if states.contains(&Review::ChangesRequested) {
        Review::ChangesRequested
    } else if states.contains(&Review::ReviewRequired) {
        Review::ReviewRequired
    } else if states.contains(&Review::Approved) {
        Review::Approved
    } else {
        Review::Unknown
    }
}

#[cfg(test)]
mod tests;
