use super::*;
use crate::integrations::test_support::{Exchange, MockApi};
use serde_json::{Value, json};

fn config() -> ConnectionConfiguration {
    [
        ("workspace".into(), "team".into()),
        ("email".into(), "user@example.com".into()),
    ]
    .into_iter()
    .collect()
}
fn repository() -> Repository {
    Repository {
        id: "repo".into(),
        owner: "team".into(),
        name: "app".into(),
        description: None,
        visibility: crate::domain::RepositoryVisibility::Private,
        web_url: "https://bitbucket.org/team/app".into(),
    }
}
fn request() -> Value {
    json!({"id":7,"title":"Update app","state":"OPEN","author":{"uuid":"alice","display_name":"Alice"},"source":{"branch":{"name":"feature"},"commit":{"hash":"abc"}},"destination":{"branch":{"name":"main"},"commit":{"hash":"def"}},"draft":false,"created_on":"2026-09-28T00:00:00Z","updated_on":"2026-09-28T00:01:00Z","links":{"html":{"href":"https://bitbucket.org/team/app/pull-requests/7"}},"summary":{"raw":"Description"},"participants":[{"user":{"uuid":"bob","display_name":"Bob"},"approved":true,"state":"approved","participated_on":"2026-09-28T00:01:00Z"}],"reviewers":[{"uuid":"bob","display_name":"Bob"}]})
}

#[tokio::test]
async fn cloud_pull_requests_include_reviews_and_native_pipeline_links() {
    let auth = "authorization: Basic dXNlckBleGFtcGxlLmNvbTpzZWNyZXQ=";
    let statuses = json!({"values":[{"key":"build","name":"Build","state":"SUCCESSFUL","url":"https://bitbucket.org/team/app/pipelines/results/12"}]});
    let api = MockApi::start(vec![
        Exchange::json(
            "/2.0/repositories/team/repo/pullrequests?state=OPEN&sort=-updated_on&pagelen=50",
            auth,
            json!({"values":[{"id":7}]}),
        ),
        Exchange::json(
            "/2.0/repositories/team/repo/pullrequests/7",
            auth,
            request(),
        ),
        Exchange::json(
            "/2.0/repositories/team/repo/pullrequests/7/statuses?pagelen=50",
            auth,
            statuses.clone(),
        ),
        Exchange::json(
            "/2.0/repositories/team/repo/pullrequests/7/activity?pagelen=50",
            auth,
            json!({"values":[]}),
        ),
        Exchange::json(
            "/2.0/repositories/team/repo/pullrequests/7",
            auth,
            request(),
        ),
        Exchange::json(
            "/2.0/repositories/team/repo/pullrequests/7/statuses?pagelen=50",
            auth,
            statuses,
        ),
        Exchange::json(
            "/2.0/repositories/team/repo/pullrequests/7/activity?pagelen=50",
            auth,
            json!({"values":[]}),
        ),
        Exchange::json(
            "/2.0/repositories/team/repo/pipelines?q=target.commit.hash%3D%22abc%22&sort=-created_on&pagelen=50",
            auth,
            json!({"values":[{"uuid":"pipeline-uuid","build_number":12,"target":{"commit":{"hash":"abc"}}}]}),
        ),
        Exchange::json(
            "/2.0/repositories/team/repo/pullrequests/7/commits?pagelen=1",
            auth,
            json!({"values":[{"hash":"abc","message":"Update app\nBody","date":"2026-09-28T00:00:00Z","author":{"raw":"Alice","user":{"uuid":"alice","display_name":"Alice"}}}]}),
        ),
    ]);
    let mut client = BitbucketClient::new().unwrap();
    client.api_base = Url::parse(&format!("{}/2.0/", api.url)).unwrap();
    let token = ProviderToken::new("secret".into());
    let requests = client
        .list_change_requests(&config(), &token, &repository())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(requests[0].review_status, Review::Approved);
    assert_eq!(
        requests[0]
            .relationships
            .reviewers
            .ids
            .first()
            .map(String::as_str),
        Some("bob")
    );
    assert!(
        requests[0]
            .relationships
            .evaluate("bob")
            .reasons
            .contains(&crate::domain::RelevanceReason::Reviewed)
    );
    assert_eq!(requests[0].merge_status, Merge::Unknown);
    let details = client
        .change_request_details(&config(), &token, &repository(), 7)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(details.change_request, requests[0]);
    assert_eq!(
        details.checks[0].workflow_run_id.as_deref(),
        Some("pipeline-uuid")
    );
    assert_eq!(details.body.as_deref(), Some("Description"));
    assert_eq!(details.latest_commit.unwrap().sha, "abc");
    api.finish();
}

#[test]
fn review_requests_and_requested_changes_are_distinct_from_approvals() {
    let mut value = request();
    value["participants"][0]["approved"] = json!(false);
    value["participants"][0]["state"] = json!("changes_requested");
    let mapped: PullRequest = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        mapped.summary(&[]).unwrap().review_status,
        Review::ChangesRequested
    );
    value["participants"] = json!([]);
    let mapped: PullRequest = serde_json::from_value(value).unwrap();
    assert_eq!(
        mapped.summary(&[]).unwrap().review_status,
        Review::ReviewRequired
    );
}

#[test]
fn cloud_lifecycle_and_missing_source_are_mapped_without_fabricated_success() {
    for (state, expected) in [
        ("MERGED", ChangeRequestState::Merged),
        ("DECLINED", ChangeRequestState::Closed),
        ("SUPERSEDED", ChangeRequestState::Closed),
    ] {
        let mut value = request();
        value["state"] = json!(state);
        value["source"] = Value::Null;
        let mapped: PullRequest = serde_json::from_value(value).unwrap();
        let summary = mapped.summary(&[]).unwrap();
        assert_eq!(summary.state, expected);
        assert_eq!(summary.check_status, Check::Unknown);
    }
}

#[tokio::test]
async fn missing_pr_is_none_but_bad_tokens_are_errors() {
    for (status, expected) in [(404, Ok(None)), (401, Err(Failure::InvalidCredentials))] {
        let mut response = Exchange::json(
            "/2.0/repositories/team/repo/pullrequests/7",
            "authorization: Basic",
            json!({}),
        );
        response.status = status;
        let api = MockApi::start(vec![response]);
        let mut client = BitbucketClient::new().unwrap();
        client.api_base = Url::parse(&format!("{}/2.0/", api.url)).unwrap();
        assert_eq!(
            client
                .change_request_details(
                    &config(),
                    &ProviderToken::new("secret".into()),
                    &repository(),
                    7
                )
                .await,
            expected
        );
        api.finish();
    }
}

#[test]
fn personal_prs_match_account_uuid_and_pending_reviewers() {
    use crate::domain::RelevanceReason::*;
    let mut value = request();
    let pull: PullRequest = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        pull.relationships().evaluate("alice").reasons,
        vec![Authored]
    );
    assert_eq!(pull.relationships().evaluate("bob").reasons, vec![Reviewed]);
    assert!(!pull.relationships().evaluate("Alice").matches());
    assert!(
        pull.relationships()
            .evaluate("unrelated")
            .reasons
            .is_empty()
    );
    value["participants"] = json!([]);
    let pull: PullRequest = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        pull.relationships().evaluate("bob").reasons,
        vec![ReviewRequested]
    );
    value["reviewers"] = Value::Null;
    let pull: PullRequest = serde_json::from_value(value).unwrap();
    assert!(!pull.relationships().requested_reviewers.complete);
}

#[tokio::test]
async fn unavailable_review_history_keeps_proven_authorship_without_failing_monitoring() {
    let auth = "authorization: Basic";
    let api = MockApi::start(vec![Exchange::denied(
        "/2.0/repositories/team/repo/pullrequests/7/activity?pagelen=50",
        auth,
    )]);
    let mut client = BitbucketClient::new().unwrap();
    client.api_base = Url::parse(&format!("{}/2.0/", api.url)).unwrap();
    let pull = serde_json::from_value(request()).unwrap();
    let result = client
        .pull_relationships(
            &config(),
            &ProviderToken::new("token".into()),
            &repository(),
            &pull,
        )
        .await;
    assert_eq!(
        result.evaluate("alice").reasons,
        vec![crate::domain::RelevanceReason::Authored]
    );
    assert!(!result.reviewers.complete);
    api.finish();
}
