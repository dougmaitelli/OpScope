use super::*;

#[test]
fn rejected_duplicate_acquisitions_do_not_release_the_running_action() {
    let active = Arc::default();
    let key = (
        "connection".into(),
        "repo".into(),
        ActionTarget::ChangeRequest {
            number: 7,
        },
    );
    let permit = ActionPermit::acquire(&active, key.clone()).unwrap();
    assert!(matches!(
        ActionPermit::acquire(&active, key.clone()),
        Err(ActionFailure::Busy)
    ));
    assert!(matches!(
        ActionPermit::acquire(&active, key.clone()),
        Err(ActionFailure::Busy)
    ));
    drop(permit);
    assert!(ActionPermit::acquire(&active, key).is_ok());
}

#[test]
fn action_contract_accepts_only_allowlisted_operations() {
    assert!(serde_json::from_str::<SourceAction>("\"dependabotRecreate\"").is_ok());
    assert!(serde_json::from_str::<SourceAction>("\"@dependabot merge\"").is_err());
    assert!(
        serde_json::from_str::<ActionTarget>(
            r#"{"type":"arbitraryUrl","url":"https://example.com"}"#
        )
        .is_err()
    );
}
