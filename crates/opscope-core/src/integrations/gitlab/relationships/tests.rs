use super::*;
use crate::domain::RelevanceReason;
use crate::integrations::test_support::{Exchange, MockApi, relevance_repository, relevance_run};
use serde_json::json;
const AUTH: &str = "private-token: token";

#[tokio::test]
async fn matches_commit_and_pr_authors_and_deduplicates_commit_lookups() {
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v4/user",
            AUTH,
            json!({"id":1,"email":"me@example.com"}),
        ),
        Exchange::json("/api/v4/user/emails?per_page=50&page=1", AUTH, json!([])),
        Exchange::json(
            "/api/v4/projects/3/repository/commits/abc",
            AUTH,
            json!({"author_email":"ME@example.com"}),
        ),
        Exchange::json(
            "/api/v4/projects/3/repository/commits/abc/merge_requests?per_page=50&page=1",
            AUTH,
            json!([{"author":{"id":1}}]),
        ),
    ]);
    let client = GitLabClient::new().unwrap();
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
                .identities
                .first()
                .map(|identity| identity.account_id.as_str()),
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
            "/api/v4/user",
            AUTH,
            json!({"id":1,"email":"me@example.com"}),
        ),
        Exchange::json("/api/v4/user/emails?per_page=50&page=1", AUTH, json!([])),
        Exchange::json(
            "/api/v4/projects/3/repository/commits/abc",
            AUTH,
            json!({"author_email":"someone@example.com"}),
        ),
        Exchange::json(
            "/api/v4/projects/3/repository/commits/abc/merge_requests?per_page=50&page=1",
            AUTH,
            json!([{"author":{"id":2}}]),
        ),
    ]);
    let client = GitLabClient::new().unwrap();
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
            "/api/v4/user",
            AUTH,
            json!({"id":1,"email":"me@example.com"}),
        ),
        Exchange::json("/api/v4/user/emails?per_page=50&page=1", AUTH, json!([])),
        Exchange::json(
            "/api/v4/projects/3/repository/commits/abc",
            AUTH,
            json!({"author_email":"ME@example.com"}),
        ),
        Exchange::denied(
            "/api/v4/projects/3/repository/commits/abc/merge_requests?per_page=50&page=1",
            AUTH,
        ),
    ]);
    let client = GitLabClient::new().unwrap();
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
async fn resource_authors_remain_reusable_when_viewer_identity_is_unavailable() {
    let api = MockApi::start(vec![
        Exchange::denied("/api/v4/user", AUTH),
        Exchange::json(
            "/api/v4/projects/3/repository/commits/abc",
            AUTH,
            json!({"author_email":"other@example.com"}),
        ),
        Exchange::json(
            "/api/v4/projects/3/repository/commits/abc/merge_requests?per_page=50&page=1",
            AUTH,
            json!([{"id":101,"iid":7,"author":{"id":2}}]),
        ),
    ]);
    let client = GitLabClient::new().unwrap();
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
    assert!(runs[0].relationships.identities.is_empty());
    assert!(!runs[0].relationships.evaluate("1").matches());
    assert_eq!(
        runs[0].relationships.evaluate("2").reasons,
        [RelevanceReason::ChangeRequestAuthored]
    );
    assert_eq!(
        runs[0].relationships.linked_change_requests[0].number,
        Some(7)
    );
    api.finish();
}

#[tokio::test]
async fn review_comments_are_found_on_later_pages_but_system_notes_are_not_reviews() {
    let path = "/api/v4/projects/3/merge_requests/7/notes?per_page=50&page=";
    let mut first = Exchange::json(
        &format!("{path}1"),
        AUTH,
        json!([{"system":true,"author":{"id":1}}]),
    );
    first.headers.push_str("X-Next-Page: 2\r\n");
    let api = MockApi::start(vec![
        first,
        Exchange::json(
            &format!("{path}2"),
            AUTH,
            json!([{"system":false,"author":{"id":1}}]),
        ),
    ]);
    let client = GitLabClient::new().unwrap();
    let config = [("serverUrl".into(), api.url.clone())]
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
        Some(vec!["1".into()])
    );
    api.finish();
}

#[tokio::test]
async fn unconfirmed_secondary_email_does_not_prove_commit_ownership() {
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v4/user",
            AUTH,
            json!({"id":1,"email":"me@example.com"}),
        ),
        Exchange::json(
            "/api/v4/user/emails?per_page=50&page=1",
            AUTH,
            json!([{"email":"unconfirmed@example.com","confirmed_at":null}]),
        ),
        Exchange::json(
            "/api/v4/projects/3/repository/commits/abc",
            AUTH,
            json!({"author_email":"unconfirmed@example.com"}),
        ),
        Exchange::json(
            "/api/v4/projects/3/repository/commits/abc/merge_requests?per_page=50&page=1",
            AUTH,
            json!([]),
        ),
    ]);
    let client = GitLabClient::new().unwrap();
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
    api.finish();
}
