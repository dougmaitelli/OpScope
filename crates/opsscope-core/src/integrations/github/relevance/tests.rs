use super::*;

#[test]
fn personal_pull_requests_include_authored_reviewed_and_requested() {
    for (field, value, expected) in [
        ("viewerDidAuthor", json!(true), RelevanceReason::Authored),
        (
            "viewerLatestReview",
            json!({"state":"APPROVED"}),
            RelevanceReason::Reviewed,
        ),
        (
            "viewerLatestReviewRequest",
            json!({"id":"request"}),
            RelevanceReason::ReviewRequested,
        ),
    ] {
        let mut node = json!({"__typename":"PullRequest", "viewerDidAuthor":false,
            "viewerLatestReview":null,"viewerLatestReviewRequest":null});
        node[field] = value;
        let relevance = item_relevance(&node);
        assert_eq!(relevance.reasons, vec![expected]);
        assert!(relevance.complete);
    }
}

#[test]
fn pending_reviews_and_missing_metadata_are_not_confirmed_matches() {
    assert!(
        !item_relevance(&json!({"__typename":"PullRequest", "viewerDidAuthor":false,
        "viewerLatestReview":{"state":"PENDING"},"viewerLatestReviewRequest":null}))
        .matches()
    );
    assert!(!item_relevance(&Value::Null).complete);
    assert_eq!(
        serde_json::from_str::<Relevance>("{}").unwrap(),
        Relevance::default()
    );
}

#[test]
fn issues_match_subscriptions_and_discussions_and_track_incomplete_pages() {
    let mut node = json!({"__typename":"Issue", "viewerDidAuthor":false,
        "viewerSubscription":"UNSUBSCRIBED", "participants":{"nodes":[],"pageInfo":{"hasNextPage":true}}});
    assert!(!item_relevance(&node).complete);
    node["participants"]["nodes"] = json!([{"isViewer":true}]);
    assert_eq!(
        item_relevance(&node).reasons,
        vec![RelevanceReason::Discussed]
    );
    node["participants"]["nodes"] = json!([]);
    node["viewerSubscription"] = json!("SUBSCRIBED");
    assert_eq!(
        item_relevance(&node).reasons,
        vec![RelevanceReason::Subscribed]
    );
    node["viewerSubscription"] = json!("IGNORED");
    node["participants"]["pageInfo"]["hasNextPage"] = json!(false);
    assert!(!item_relevance(&node).matches());
    assert!(item_relevance(&node).complete);
}

#[test]
fn workflow_ownership_uses_commits_or_pull_requests_not_the_trigger_actor() {
    let mut commit = json!({"author":{"user":{"isViewer":false}},
        "associatedPullRequests":{"nodes":[],"pageInfo":{"hasNextPage":false}}});
    assert!(!commit_relevance(&commit, &[]).matches());
    commit["author"]["user"]["isViewer"] = json!(true);
    assert_eq!(
        commit_relevance(&commit, &[]).reasons,
        vec![RelevanceReason::CommitAuthored]
    );
    assert_eq!(
        commit_relevance(&Value::Null, &[&json!({"viewerDidAuthor":true})]).reasons,
        vec![RelevanceReason::ChangeRequestAuthored]
    );
    assert!(!commit_relevance(&Value::Null, &[]).complete);
}
