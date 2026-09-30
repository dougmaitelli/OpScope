use super::*;

#[test]
fn the_same_cached_evidence_can_be_evaluated_for_different_accounts() {
    let facts = Relationships {
        authors: AccountSet::new(["alice".into()], true),
        reviewers: AccountSet::new(["bob".into()], true),
        ..Default::default()
    };
    assert_eq!(facts.evaluate("alice").reasons, [RelevanceReason::Authored]);
    assert_eq!(facts.evaluate("bob").reasons, [RelevanceReason::Reviewed]);
    assert!(!facts.evaluate("carol").matches());
    let json = serde_json::to_string(&facts).unwrap();
    assert!(!json.contains("reasons"));
    assert_eq!(serde_json::from_str::<Relationships>(&json).unwrap(), facts);
}

#[test]
fn viewer_observations_are_scoped_and_missing_evidence_stays_unknown() {
    let facts = Relationships {
        subscribers: AccountSet::viewer("alice", Some(true)),
        ..Default::default()
    };
    assert!(facts.evaluate("alice").matches());
    assert!(!facts.evaluate("bob").matches());
    assert_eq!(facts.subscribers.contains("bob"), None);
    assert_eq!(facts.review_requested("alice"), None);
    assert_eq!(AccountSet::new([], true).contains("alice"), Some(false));
}

#[test]
fn email_evidence_requires_a_provider_confirmed_identity() {
    let facts = Relationships {
        commit_author_email: Some("Alice@Example.com".into()),
        identities: vec![AccountEmails {
            account_id: "alice".into(),
            emails: vec!["alice@example.com".into()],
            complete: true,
        }],
        ..Default::default()
    };
    assert!(facts.evaluate("alice").matches());
    assert!(!facts.evaluate("bob").matches());
}

#[test]
fn review_requests_are_evaluated_from_assignments_and_decisions() {
    let mut facts = Relationships {
        assigned_reviewers: AccountSet::new(["alice".into(), "bob".into()], true),
        completed_reviewers: AccountSet::new(["bob".into()], true),
        ..Default::default()
    };
    assert_eq!(facts.review_requested("alice"), Some(true));
    assert_eq!(facts.review_requested("bob"), Some(false));
    assert_eq!(facts.review_requested("carol"), Some(false));
    assert!(
        facts
            .evaluate("alice")
            .reasons
            .contains(&RelevanceReason::ReviewRequested)
    );
    facts.completed_reviewers.complete = false;
    assert_eq!(facts.review_requested("alice"), None);
    assert_eq!(facts.review_requested("bob"), Some(false));
}
