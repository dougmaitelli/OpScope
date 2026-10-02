use super::*;
use crate::domain::RelevanceReason;
use crate::integrations::test_support::{Exchange, MockApi, relevance_repository, relevance_run};
use serde_json::json;
const AUTH: &str = "authorization: Basic";

#[tokio::test]
async fn matches_commit_and_pr_authors_and_deduplicates_commit_lookups() {
    let api = MockApi::start(vec![
        Exchange::json(
            "/2.0/repositories/team/3/commit/abc",
            AUTH,
            json!({"author":{"user":{"uuid":"me"}}}),
        ),
        Exchange::json(
            "/2.0/repositories/team/3/commit/abc/pullrequests?pagelen=50",
            AUTH,
            json!({"values":[{"author":{"uuid":"me"}}]}),
        ),
    ]);
    let mut client = BitbucketClient::new().unwrap();
    client.api_base = Url::parse(&format!("{}/2.0/", api.url)).unwrap();
    let config = [
        ("workspace".into(), "team".into()),
        ("email".into(), "me@example.com".into()),
    ]
    .into_iter()
    .collect();
    let mut runs = vec![relevance_run("abc"), relevance_run("abc")];
    client
        .run_relationships(
            &config,
            &ProviderToken::new("token".into()),
            &relevance_repository(),
            &mut runs,
        )
        .await;
    for run in &runs {
        assert_eq!(
            run.relationships
                .commit_authors
                .ids
                .first()
                .map(String::as_str),
            Some("me")
        );
        assert_eq!(
            run.relationships.evaluate("me").reasons,
            vec![
                RelevanceReason::CommitAuthored,
                RelevanceReason::ChangeRequestAuthored
            ]
        );
        assert!(run.relationships.change_request_authors.complete);
    }
    api.finish();
}

#[tokio::test]
async fn triggering_a_run_does_not_make_it_personal() {
    let api = MockApi::start(vec![
        Exchange::json(
            "/2.0/repositories/team/3/commit/abc",
            AUTH,
            json!({"author":{"user":{"uuid":"other"}}}),
        ),
        Exchange::json(
            "/2.0/repositories/team/3/commit/abc/pullrequests?pagelen=50",
            AUTH,
            json!({"values":[{"author":{"uuid":"other"}}]}),
        ),
    ]);
    let mut client = BitbucketClient::new().unwrap();
    client.api_base = Url::parse(&format!("{}/2.0/", api.url)).unwrap();
    let config = [
        ("workspace".into(), "team".into()),
        ("email".into(), "me@example.com".into()),
    ]
    .into_iter()
    .collect();
    let mut runs = vec![relevance_run("abc")];
    client
        .run_relationships(
            &config,
            &ProviderToken::new("token".into()),
            &relevance_repository(),
            &mut runs,
        )
        .await;
    assert!(!runs[0].relationships.evaluate("me").matches());
    assert!(runs[0].relationships.change_request_authors.complete);
    api.finish();
}

#[tokio::test]
async fn inaccessible_associations_preserve_proven_authorship_but_not_completeness() {
    let api = MockApi::start(vec![
        Exchange::json(
            "/2.0/repositories/team/3/commit/abc",
            AUTH,
            json!({"author":{"user":{"uuid":"me"}}}),
        ),
        Exchange::denied(
            "/2.0/repositories/team/3/commit/abc/pullrequests?pagelen=50",
            AUTH,
        ),
    ]);
    let mut client = BitbucketClient::new().unwrap();
    client.api_base = Url::parse(&format!("{}/2.0/", api.url)).unwrap();
    let config = [
        ("workspace".into(), "team".into()),
        ("email".into(), "me@example.com".into()),
    ]
    .into_iter()
    .collect();
    let mut runs = vec![relevance_run("abc")];
    client
        .run_relationships(
            &config,
            &ProviderToken::new("token".into()),
            &relevance_repository(),
            &mut runs,
        )
        .await;
    assert_eq!(
        runs[0].relationships.evaluate("me").reasons,
        vec![RelevanceReason::CommitAuthored]
    );
    assert!(!runs[0].relationships.change_request_authors.complete);
    api.finish();
}

#[tokio::test]
async fn unavailable_relationship_endpoints_leave_evidence_unknown() {
    let api = MockApi::start(vec![
        Exchange::denied("/2.0/repositories/team/3/commit/abc", AUTH),
        Exchange::denied(
            "/2.0/repositories/team/3/commit/abc/pullrequests?pagelen=50",
            AUTH,
        ),
    ]);
    let mut client = BitbucketClient::new().unwrap();
    client.api_base = Url::parse(&format!("{}/2.0/", api.url)).unwrap();
    let config = [
        ("workspace".into(), "team".into()),
        ("email".into(), "me@example.com".into()),
    ]
    .into_iter()
    .collect();
    let mut runs = vec![relevance_run("abc")];
    client
        .run_relationships(
            &config,
            &ProviderToken::new("token".into()),
            &relevance_repository(),
            &mut runs,
        )
        .await;
    assert_eq!(runs[0].relationships, Relationships::default());
    api.finish();
}

#[tokio::test]
async fn submitted_review_comments_count_but_deleted_comments_and_unrelated_updates_do_not() {
    for (events, expected) in [
        (
            json!([{"comment":{"user":{"uuid":"me"},"deleted":false}}]),
            true,
        ),
        (
            json!([{"comment":{"user":{"uuid":"me"},"deleted":true}},{"update":{"author":{"uuid":"me"}}}]),
            false,
        ),
        (json!([{"changes_requested":{"user":{"uuid":"me"}}}]), true),
    ] {
        let api = MockApi::start(vec![Exchange::json(
            "/2.0/repositories/team/3/pullrequests/7/activity?pagelen=50",
            AUTH,
            json!({"values":events}),
        )]);
        let mut client = BitbucketClient::new().unwrap();
        client.api_base = Url::parse(&format!("{}/2.0/", api.url)).unwrap();
        let config = [
            ("workspace".into(), "team".into()),
            ("email".into(), "me@example.com".into()),
        ]
        .into_iter()
        .collect();
        assert_eq!(
            client
                .review_history(
                    &config,
                    &ProviderToken::new("token".into()),
                    &relevance_repository(),
                    7
                )
                .await
                .map(|set| set.ids),
            Some(if expected { vec!["me".into()] } else { vec![] })
        );
        api.finish();
    }
}
