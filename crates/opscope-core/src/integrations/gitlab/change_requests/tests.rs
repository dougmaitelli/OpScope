use super::*;
use crate::integrations::test_support::{Exchange, MockApi};
use serde_json::{Value, json};

fn request() -> Value {
    json!({"id":101,"iid":7,"title":"Update app","state":"opened","author":{"username":"alice"},"source_branch":"feature","target_branch":"main","draft":false,"detailed_merge_status":"mergeable","head_pipeline":{"id":50,"project_id":3,"status":"success","web_url":"https://gitlab.com/team/app/-/pipelines/50"},"created_at":"2026-09-28T00:00:00Z","updated_at":"2026-09-28T00:01:00Z","web_url":"https://gitlab.com/team/app/-/merge_requests/7","description":"Description","labels":["feature"],"reviewers":[{"username":"bob"}],"source_project_id":3,"sha":"abc"})
}
fn repository() -> Repository {
    Repository {
        id: "3".into(),
        owner: "team".into(),
        name: "app".into(),
        description: None,
        visibility: crate::domain::RepositoryVisibility::Private,
        web_url: "https://gitlab.com/team/app".into(),
    }
}

#[tokio::test]
async fn lists_and_opens_merge_requests_with_reviews_checks_and_commit() {
    let auth = "private-token: token";
    let approvals = json!({"approvals_left":0,"approved_by":[{"user":{"id":2,"username":"bob"}}]});
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v4/projects/3/merge_requests?state=opened&scope=all&order_by=updated_at&sort=desc&per_page=50&page=1",
            auth,
            json!([request()]),
        ),
        Exchange::json("/api/v4/projects/3/merge_requests/7", auth, request()),
        Exchange::json(
            "/api/v4/projects/3/merge_requests/7/approvals",
            auth,
            approvals.clone(),
        ),
        Exchange::json(
            "/api/v4/projects/3/merge_requests/7/notes?per_page=50&page=1",
            auth,
            json!([]),
        ),
        Exchange::json("/api/v4/projects/3/merge_requests/7", auth, request()),
        Exchange::json(
            "/api/v4/projects/3/merge_requests/7/approvals",
            auth,
            approvals,
        ),
        Exchange::json(
            "/api/v4/projects/3/merge_requests/7/notes?per_page=50&page=1",
            auth,
            json!([]),
        ),
        Exchange::json(
            "/api/v4/projects/3/repository/commits/abc",
            auth,
            json!({"id":"abc","title":"Update app","author_name":"Alice","committed_date":"2026-09-28T00:00:00Z"}),
        ),
    ]);
    let client = GitLabClient::new().unwrap();
    let config = [("serverUrl".into(), api.url.clone())]
        .into_iter()
        .collect();
    let token = ProviderToken::new("token".into());
    let requests = client
        .list_change_requests(&config, &token, &repository())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(requests[0].number, 7);
    assert_eq!(requests[0].id, "101");
    assert_eq!(requests[0].review_status, Review::Approved);
    assert_eq!(
        requests[0]
            .relationships
            .reviewers
            .ids
            .first()
            .map(String::as_str),
        Some("2")
    );
    assert!(
        requests[0]
            .relationships
            .evaluate("2")
            .reasons
            .contains(&crate::domain::RelevanceReason::Reviewed)
    );
    assert_eq!(requests[0].check_status, Check::Passed);
    let details = client
        .change_request_details(&config, &token, &repository(), 7)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(details.change_request, requests[0]);
    assert_eq!(details.checks[0].workflow_run_id.as_deref(), Some("50"));
    assert_eq!(details.reviews[0].reviewer.as_deref(), Some("bob"));
    assert_eq!(details.latest_commit.unwrap().sha, "abc");
    assert_eq!(details.labels, vec!["feature"]);
    api.finish();
}

#[test]
fn lifecycle_draft_and_unknown_checks_are_not_guessed() {
    for (state, expected) in [
        ("opened", ChangeRequestState::Open),
        ("merged", ChangeRequestState::Merged),
        ("closed", ChangeRequestState::Closed),
    ] {
        let mut value = request();
        value["state"] = json!(state);
        value["head_pipeline"] = Value::Null;
        let mapped: MergeRequest = serde_json::from_value(value).unwrap();
        let summary = mapped.summary(None).unwrap();
        assert_eq!(summary.state, expected);
        assert_eq!(summary.check_status, Check::None);
        assert_eq!(summary.review_status, Review::Unknown);
    }
    let mut value = request();
    value["draft"] = json!(true);
    value["detailed_merge_status"] = json!("requested_changes");
    let mapped: MergeRequest = serde_json::from_value(value).unwrap();
    let summary = mapped.summary(None).unwrap();
    assert!(summary.draft);
    assert_eq!(summary.review_status, Review::ChangesRequested);
    assert_eq!(summary.merge_status, Merge::Blocked);
}

#[tokio::test]
async fn unavailable_approvals_do_not_claim_approval_and_fork_pipelines_stay_external() {
    let auth = "private-token: token";
    let mut value = request();
    value["source_project_id"] = Value::Null;
    value["head_pipeline"]["project_id"] = json!(99);
    let mut unavailable = Exchange::json(
        "/api/v4/projects/3/merge_requests/7/approvals",
        auth,
        json!({}),
    );
    unavailable.status = 403;
    let api = MockApi::start(vec![
        Exchange::json("/api/v4/projects/3/merge_requests/7", auth, value),
        unavailable,
    ]);
    let client = GitLabClient::new().unwrap();
    let config = [("serverUrl".into(), api.url.clone())]
        .into_iter()
        .collect();
    let details = client
        .change_request_details(
            &config,
            &ProviderToken::new("token".into()),
            &repository(),
            7,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(details.change_request.review_status, Review::Unknown);
    assert_eq!(details.checks[0].workflow_run_id, None);
    api.finish();
}

#[test]
fn known_empty_approvals_differ_from_unavailable_approvals() {
    let mut value = request();
    value["head_pipeline"]["status"] = json!("future-state");
    let mapped: MergeRequest = serde_json::from_value(value).unwrap();
    let empty = Approvals {
        approvals_left: Some(0),
        approved_by: vec![],
    };
    let summary = mapped.summary(Some(&empty)).unwrap();
    assert_eq!(summary.check_status, Check::Unknown);
    assert_eq!(summary.review_status, Review::None);
    assert_eq!(mapped.summary(None).unwrap().review_status, Review::Unknown);
}

#[tokio::test]
async fn pagination_failure_is_not_published_as_a_partial_open_set() {
    let auth = "private-token: token";
    let mut first = Exchange::json(
        "/api/v4/projects/3/merge_requests?state=opened&scope=all&order_by=updated_at&sort=desc&per_page=50&page=1",
        auth,
        json!([request()]),
    );
    first.headers.push_str("X-Next-Page: 2\r\n");
    let mut second = Exchange::json(
        "/api/v4/projects/3/merge_requests?state=opened&scope=all&order_by=updated_at&sort=desc&per_page=50&page=2",
        auth,
        json!({}),
    );
    second.status = 429;
    let api = MockApi::start(vec![first, second]);
    let client = GitLabClient::new().unwrap();
    let config = [("serverUrl".into(), api.url.clone())]
        .into_iter()
        .collect();
    assert_eq!(
        client
            .list_change_requests(&config, &ProviderToken::new("token".into()), &repository())
            .await,
        Err(Failure::RateLimited)
    );
    api.finish();
}

#[test]
fn personal_merge_requests_match_ids_and_separate_approval_from_review_requests() {
    use crate::domain::RelevanceReason::*;
    let mut value = request();
    value["author"]["id"] = json!(1);
    value["reviewers"][0]["id"] = json!(2);
    let pull: MergeRequest = serde_json::from_value(value).unwrap();
    let approvals: Approvals =
        serde_json::from_value(json!({"approved_by":[],"approvals_left":1})).unwrap();
    assert_eq!(
        pull.relationships(Some(&approvals)).evaluate("1").reasons,
        vec![Authored]
    );
    assert_eq!(
        pull.relationships(Some(&approvals)).evaluate("2").reasons,
        vec![ReviewRequested]
    );
    assert!(!pull.relationships(Some(&approvals)).evaluate("3").matches());
    let approvals = serde_json::from_value(
        json!({"approved_by":[{"user":{"id":2,"username":"bob"}}],"approvals_left":0}),
    )
    .unwrap();
    assert_eq!(
        pull.relationships(Some(&approvals)).evaluate("2").reasons,
        vec![Reviewed]
    );
    assert!(
        pull.relationships(Some(&approvals))
            .evaluate("unrelated")
            .reasons
            .is_empty()
    );
    assert!(!pull.relationships(None).participants.complete);
}
