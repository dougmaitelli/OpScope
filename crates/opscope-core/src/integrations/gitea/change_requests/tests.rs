use super::*;
use crate::integrations::test_support::{Exchange, MockApi};
use serde_json::{Value, json};

fn request() -> Value {
    json!({"id":101,"number":7,"title":"Update app","state":"open","user":{"id":1,"login":"alice"},"head":{"ref":"feature","sha":"abc"},"base":{"ref":"main","sha":"def"},"merged":false,"mergeable":true,"draft":false,"created_at":"2026-09-28T00:00:00Z","updated_at":"2026-09-28T00:01:00Z","html_url":"https://gitea.com/team/app/pulls/7","body":"Description","labels":[{"name":"feature"}],"requested_reviewers":[]})
}
fn repository() -> Repository {
    Repository {
        id: "3".into(),
        owner: "team".into(),
        name: "app".into(),
        description: None,
        visibility: crate::domain::RepositoryVisibility::Private,
        web_url: "https://gitea.com/team/app".into(),
    }
}
fn review(id: u64, state: &str) -> Value {
    json!({"id":id,"user":{"id":2,"login":"bob"},"state":state,"dismissed":false,"stale":false,"submitted_at":"2026-09-28T00:01:00Z"})
}

#[test]
fn latest_decisions_override_old_reviews_without_comments_erasing_approval() {
    let reviews = serde_json::from_value(json!([
        review(1, "REQUEST_CHANGES"),
        review(2, "APPROVED"),
        review(3, "COMMENT")
    ]))
    .unwrap();
    let current = current_reviews(reviews, &[]);
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].status, Review::Approved);
    let mut stale = review(4, "APPROVED");
    stale["stale"] = json!(true);
    let current = current_reviews(
        serde_json::from_value(json!([review(1, "APPROVED"), stale])).unwrap(),
        &[],
    );
    assert_eq!(current[0].status, Review::Unknown);
    let current = current_reviews(
        serde_json::from_value(json!([review(1, "APPROVED")])).unwrap(),
        &[Person {
            id: 2,
            login: "bob".into(),
        }],
    );
    assert_eq!(current[0].status, Review::ReviewRequired);
}

#[tokio::test]
async fn lists_pr_signals_and_resolves_check_run_ids_from_provider_metadata() {
    let auth = "authorization: token token";
    let reviews = json!([review(1, "APPROVED")]);
    let status = json!({"state":"success","total_count":1,"statuses":[{"context":"build","status":"success","target_url":"https://gitea.com/team/app/actions/runs/7/jobs/8"}]});
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v1/repos/team/app/pulls?state=open&sort=recentupdate&limit=50&page=1",
            auth,
            json!([request()]),
        ),
        Exchange::json(
            "/api/v1/repos/team/app/pulls/7/reviews?limit=50&page=1",
            auth,
            reviews.clone(),
        ),
        Exchange::json(
            "/api/v1/repos/team/app/commits/abc/status?limit=50&page=1",
            auth,
            status.clone(),
        ),
        Exchange::json("/api/v1/repos/team/app/pulls/7", auth, request()),
        Exchange::json(
            "/api/v1/repos/team/app/pulls/7/reviews?limit=50&page=1",
            auth,
            reviews,
        ),
        Exchange::json(
            "/api/v1/repos/team/app/commits/abc/status?limit=50&page=1",
            auth,
            status,
        ),
        Exchange::json(
            "/api/v1/repos/team/app/actions/runs?head_sha=abc&limit=50&page=1",
            auth,
            json!({"total_count":1,"workflow_runs":[{"id":70,"head_sha":"abc","html_url":"https://gitea.com/team/app/actions/runs/7"}]}),
        ),
        Exchange::json(
            "/api/v1/repos/team/app/git/commits/abc",
            auth,
            json!({"sha":"abc","commit":{"message":"Update app\nBody","author":{"name":"Alice","date":"2026-09-28T00:00:00Z"},"committer":{"name":"Alice","date":"2026-09-28T00:00:00Z"}}}),
        ),
    ]);
    let client = GiteaClient::new().unwrap();
    let config = [("serverUrl".into(), api.url.clone())]
        .into_iter()
        .collect();
    let token = ProviderToken::new("token".into());
    let requests = client
        .list_change_requests(&config, &token, &repository())
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
    assert_eq!(requests[0].merge_status, Merge::Unknown);
    let details = client
        .change_request_details(&config, &token, &repository(), 7)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(details.change_request, requests[0]);
    assert_eq!(details.checks[0].workflow_run_id.as_deref(), Some("70"));
    assert_eq!(details.latest_commit.unwrap().title, "Update app");
    api.finish();
}

#[test]
fn merged_requests_are_not_reported_as_closed_and_missing_heads_are_supported() {
    let mut value = request();
    value["state"] = json!("closed");
    value["merged"] = json!(true);
    value["head"] = Value::Null;
    let mapped: PullRequest = serde_json::from_value(value).unwrap();
    let summary = mapped.summary(&[], Check::Unknown).unwrap();
    assert_eq!(summary.state, ChangeRequestState::Merged);
    assert_eq!(summary.check_status, Check::Unknown);
    assert_eq!(summary.source_branch, "");
}

#[tokio::test]
async fn combined_api_state_is_used_instead_of_recalculating_from_check_pages() {
    let auth = "authorization: token token";
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v1/repos/team/app/pulls?state=open&sort=recentupdate&limit=50&page=1",
            auth,
            json!([request()]),
        ),
        Exchange::json(
            "/api/v1/repos/team/app/pulls/7/reviews?limit=50&page=1",
            auth,
            json!([]),
        ),
        Exchange::json(
            "/api/v1/repos/team/app/commits/abc/status?limit=50&page=1",
            auth,
            json!({"state":"pending","total_count":2,"statuses":[{"context":"lint","status":"success"}]}),
        ),
        Exchange::json(
            "/api/v1/repos/team/app/commits/abc/status?limit=50&page=2",
            auth,
            json!({"state":"pending","total_count":2,"statuses":[{"context":"test","status":"failure"}]}),
        ),
    ]);
    let client = GiteaClient::new().unwrap();
    let config = [("serverUrl".into(), api.url.clone())]
        .into_iter()
        .collect();
    let requests = client
        .list_change_requests(&config, &ProviderToken::new("token".into()), &repository())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(requests[0].check_status, Check::Running);
    api.finish();
}

#[test]
fn combined_status_mapping_preserves_unknown_states() {
    for (state, expected) in [
        ("success", Check::Passed),
        ("failure", Check::Failing),
        ("error", Check::Failing),
        ("pending", Check::Running),
        ("", Check::Unknown),
        ("future-state", Check::Unknown),
    ] {
        assert_eq!(check_state(state), expected);
    }
    let missing: CombinedStatus =
        serde_json::from_value(json!({"total_count":0,"statuses":[]})).unwrap();
    assert_eq!(check_state(&missing.state), Check::Unknown);
    assert_eq!(missing.check_status(), Check::None);
}

#[test]
fn current_gitea_reviews_prioritize_changes_and_outstanding_requests() {
    let make = |status| ChangeRequestReview {
        reviewer: None,
        status,
        submitted_at: None,
    };
    assert_eq!(review_state(&[]), Review::None);
    assert_eq!(review_state(&[make(Review::Unknown)]), Review::Unknown);
    assert_eq!(review_state(&[make(Review::Approved)]), Review::Approved);
    assert_eq!(
        review_state(&[make(Review::Approved), make(Review::ReviewRequired)]),
        Review::ReviewRequired
    );
    assert_eq!(
        review_state(&[make(Review::ReviewRequired), make(Review::ChangesRequested)]),
        Review::ChangesRequested
    );
}

#[test]
fn personal_prs_match_ids_and_submitted_reviews_not_drafts_or_display_names() {
    use crate::domain::RelevanceReason::*;
    let mut value = request();
    value["requested_reviewers"] = json!([{"id":3,"login":"alice"}]);
    let pull: PullRequest = serde_json::from_value(value).unwrap();
    let mut dismissed = review(2, "APPROVED");
    dismissed["dismissed"] = json!(true);
    let reviews: Vec<PullReview> = serde_json::from_value(json!([
        review(1, "COMMENT"),
        dismissed,
        review(3, "PENDING")
    ]))
    .unwrap();
    assert_eq!(
        pull.relationships(&reviews).evaluate("1").reasons,
        vec![Authored]
    );
    assert_eq!(
        pull.relationships(&reviews).evaluate("2").reasons,
        vec![Reviewed]
    );
    assert_eq!(
        pull.relationships(&reviews).evaluate("3").reasons,
        vec![ReviewRequested]
    );
    assert!(!pull.relationships(&reviews).evaluate("4").matches());
    assert!(
        pull.relationships(&reviews)
            .evaluate("unrelated")
            .reasons
            .is_empty()
    );
    let draft: Vec<PullReview> = serde_json::from_value(json!([review(4, "PENDING")])).unwrap();
    assert!(!pull.relationships(&draft).evaluate("2").matches());
}
