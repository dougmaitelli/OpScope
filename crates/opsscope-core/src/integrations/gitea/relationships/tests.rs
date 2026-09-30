use super::*;
use crate::domain::RelevanceReason;
use crate::integrations::test_support::{Exchange, MockApi, relevance_repository, relevance_run};
use serde_json::json;
const AUTH: &str = "authorization: token token";

#[tokio::test]
async fn matches_commit_and_pr_authors_and_deduplicates_commit_lookups() {
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v1/repos/team/app/git/commits/abc",
            AUTH,
            json!({"author":{"id":1}}),
        ),
        Exchange::json(
            "/api/v1/repos/team/app/commits/abc/pull",
            AUTH,
            json!({"user":{"id":1}}),
        ),
    ]);
    let client = GiteaClient::new().unwrap();
    let config = [("serverUrl".into(), api.url.clone())]
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
            Some("1")
        );
        assert_eq!(
            run.relationships.evaluate("1").reasons,
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
            "/api/v1/repos/team/app/git/commits/abc",
            AUTH,
            json!({"author":{"id":2}}),
        ),
        Exchange::json(
            "/api/v1/repos/team/app/commits/abc/pull",
            AUTH,
            json!({"user":{"id":2}}),
        ),
    ]);
    let client = GiteaClient::new().unwrap();
    let config = [("serverUrl".into(), api.url.clone())]
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
    assert!(!runs[0].relationships.evaluate("1").matches());
    assert!(runs[0].relationships.change_request_authors.complete);
    api.finish();
}

#[tokio::test]
async fn inaccessible_associations_preserve_proven_authorship_but_not_completeness() {
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v1/repos/team/app/git/commits/abc",
            AUTH,
            json!({"author":{"id":1}}),
        ),
        Exchange::denied("/api/v1/repos/team/app/commits/abc/pull", AUTH),
    ]);
    let client = GiteaClient::new().unwrap();
    let config = [("serverUrl".into(), api.url.clone())]
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
        runs[0].relationships.evaluate("1").reasons,
        vec![RelevanceReason::CommitAuthored]
    );
    assert!(!runs[0].relationships.change_request_authors.complete);
    api.finish();
}

#[tokio::test]
async fn unavailable_relationship_endpoints_leave_evidence_unknown() {
    let api = MockApi::start(vec![
        Exchange::denied("/api/v1/repos/team/app/git/commits/abc", AUTH),
        Exchange::denied("/api/v1/repos/team/app/commits/abc/pull", AUTH),
    ]);
    let client = GiteaClient::new().unwrap();
    let config = [("serverUrl".into(), api.url.clone())]
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
