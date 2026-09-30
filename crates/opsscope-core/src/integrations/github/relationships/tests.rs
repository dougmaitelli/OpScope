use super::*;
use crate::domain::RelevanceReason;

#[test]
fn actor_facts_can_be_reused_for_other_accounts() {
    let node = json!({"__typename":"PullRequest", "author":{"databaseId":1},
        "reviews":{"nodes":[{"state":"APPROVED","author":{"databaseId":2}}],"pageInfo":{"hasNextPage":false}},
        "reviewRequests":{"nodes":[{"requestedReviewer":{"databaseId":3}}],"pageInfo":{"hasNextPage":false}},
        "viewerDidAuthor":true,"viewerLatestReview":null,"viewerLatestReviewRequest":null});
    let facts = item_relationships(&node, "1");
    assert_eq!(facts.evaluate("1").reasons, [RelevanceReason::Authored]);
    assert_eq!(facts.evaluate("2").reasons, [RelevanceReason::Reviewed]);
    assert_eq!(
        facts.evaluate("3").reasons,
        [RelevanceReason::ReviewRequested]
    );
    assert!(!facts.evaluate("4").matches());
}

#[test]
fn pending_reviews_and_missing_metadata_are_not_confirmed_matches() {
    let facts = item_relationships(
        &json!({"__typename":"PullRequest","viewerDidAuthor":false,
        "viewerLatestReview":{"state":"PENDING"},"viewerLatestReviewRequest":null}),
        "1",
    );
    assert!(!facts.evaluate("1").matches());
    assert_eq!(facts.viewers[0].reviewed, None);
    assert_eq!(
        item_relationships(&Value::Null, "1").review_requested("1"),
        None
    );
    assert_eq!(
        serde_json::from_str::<Relationships>("{}").unwrap(),
        Relationships::default()
    );
}

#[test]
fn subscriptions_are_viewer_scoped_and_participant_pages_can_be_incomplete() {
    let node = json!({"__typename":"Issue","viewerDidAuthor":false,"viewerSubscription":"SUBSCRIBED",
        "participants":{"nodes":[{"databaseId":2,"isViewer":false}],"pageInfo":{"hasNextPage":true}}});
    let facts = item_relationships(&node, "1");
    assert_eq!(facts.evaluate("1").reasons, [RelevanceReason::Subscribed]);
    assert_eq!(facts.evaluate("2").reasons, [RelevanceReason::Discussed]);
    assert_eq!(facts.participants.contains("3"), None);
}

#[test]
fn workflow_ownership_uses_commits_or_pull_requests_not_the_trigger_actor() {
    let commit = json!({"author":{"user":{"databaseId":1,"isViewer":false}},
        "associatedPullRequests":{"nodes":[{"author":{"databaseId":2},"viewerDidAuthor":false}],"pageInfo":{"hasNextPage":false}}});
    let facts = commit_relationships(&commit, &[], "3");
    assert_eq!(
        facts.evaluate("1").reasons,
        [RelevanceReason::CommitAuthored]
    );
    assert_eq!(
        facts.evaluate("2").reasons,
        [RelevanceReason::ChangeRequestAuthored]
    );
    assert!(!facts.evaluate("3").matches());
    assert_eq!(
        commit_relationships(&Value::Null, &[], "3")
            .commit_authors
            .contains("3"),
        None
    );
}

#[tokio::test]
async fn participants_are_paged_even_when_the_viewer_already_authored_the_issue() {
    use crate::integrations::test_support::{Exchange, MockApi};
    for fail_page in [false, true] {
        let auth = "authorization: Bearer token";
        let api = MockApi::start(vec![
            Exchange::graphql(
                "/api/graphql",
                auth,
                json!({"ids":["I_1"]}),
                &[
                    "author { ... on User { databaseId } }",
                    "databaseId isViewer",
                ],
                json!({"data":{"viewer":{"databaseId":1,"login":"alice"},"nodes":[{
                    "__typename":"Issue","id":"I_1","author":{"databaseId":1},
                    "viewerDidAuthor":true,"viewerSubscription":"UNSUBSCRIBED",
                    "participants":{"nodes":[{"databaseId":2,"isViewer":false}],"pageInfo":{"hasNextPage":true,"endCursor":"next"}}
                }]}}),
            ),
            Exchange::graphql(
                "/api/graphql",
                auth,
                json!({"id":"I_1","after":"next"}),
                &[
                    "participants(first: 100, after: $after)",
                    "databaseId isViewer",
                ],
                if fail_page {
                    json!({"errors":[{"message":"Unavailable"}]})
                } else {
                    json!({"data":{"node":{"participants":{"nodes":[{"databaseId":3,"isViewer":false}],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}})
                },
            ),
        ]);
        let client = GitHubClient::new().unwrap();
        let config = [(SERVER_URL_KEY.into(), api.url.clone())]
            .into_iter()
            .collect();
        let facts = items(
            &client,
            &config,
            &ProviderToken::new("token".into()),
            &["I_1".into()],
        )
        .await;
        let facts = &facts["I_1"];
        assert_eq!(facts.evaluate("1").reasons, [RelevanceReason::Authored]);
        assert_eq!(facts.evaluate("2").reasons, [RelevanceReason::Discussed]);
        assert_eq!(
            facts.participants.contains("3"),
            if fail_page { None } else { Some(true) }
        );
        assert_eq!(facts.participants.complete, !fail_page);
        api.finish();
    }
}
