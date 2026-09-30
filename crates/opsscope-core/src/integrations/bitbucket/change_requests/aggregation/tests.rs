use super::*;

#[test]
fn missing_or_unknown_checks_do_not_mean_success() {
    assert_eq!(checks([]), Check::Unknown);
    assert_eq!(checks([Check::Passed, Check::Unknown]), Check::Unknown);
    assert_eq!(checks([Check::Passed, Check::Running]), Check::Running);
    assert_eq!(checks([Check::Running, Check::Failing]), Check::Failing);
    assert_eq!(checks([Check::Passed]), Check::Passed);
}

#[test]
fn requested_reviews_and_changes_take_priority_over_approval() {
    assert_eq!(reviews([]), Review::Unknown);
    assert_eq!(
        reviews([Review::Approved, Review::ReviewRequired]),
        Review::ReviewRequired
    );
    assert_eq!(
        reviews([Review::Approved, Review::ChangesRequested]),
        Review::ChangesRequested
    );
}
