use super::*;
use crate::domain::RelevanceReason;
use crate::integrations::test_support::{Exchange, MockApi, relevance_repository};
use serde_json::{Value, json};
const AUTH: &str = "private-token: token";

fn issue() -> Value {
    json!({"id":101,"iid":7,"title":"Fix build","state":"opened","author":{"id":1,"username":"alice"},"assignees":[{"id":2,"username":"bob"}],"labels":["bug"],"user_notes_count":2,"description":"Issue description","milestone":{"title":"Next"},"subscribed":true,"created_at":"2026-09-28T00:00:00Z","updated_at":"2026-09-28T00:01:00Z","web_url":"https://gitlab.com/team/app/-/issues/7"})
}
fn comment() -> Value {
    json!({"id":10,"author":{"id":1,"username":"alice"},"body":"A discussion comment","system":false,"created_at":"2026-09-28T00:01:00Z","updated_at":"2026-09-28T00:01:00Z"})
}

#[tokio::test]
async fn lists_and_opens_issues_with_personal_relevance_and_comments() {
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v4/projects/3",
            AUTH,
            json!({"issues_access_level":"enabled"}),
        ),
        Exchange::json(
            "/api/v4/projects/3/issues?state=opened&scope=all&order_by=updated_at&sort=desc&per_page=50&page=1",
            AUTH,
            json!([issue()]),
        ),
        Exchange::json("/api/v4/user", AUTH, json!({"id":1})),
        Exchange::json(
            "/api/v4/projects/3/issues/7/notes?order_by=created_at&sort=asc&per_page=50&page=1",
            AUTH,
            json!([comment()]),
        ),
        Exchange::json(
            "/api/v4/projects/3",
            AUTH,
            json!({"issues_access_level":"enabled"}),
        ),
        Exchange::json("/api/v4/projects/3/issues/7", AUTH, issue()),
        Exchange::json("/api/v4/user", AUTH, json!({"id":1})),
        Exchange::json(
            "/api/v4/projects/3/issues/7/notes?order_by=created_at&sort=asc&per_page=50&page=1",
            AUTH,
            {
                let mut system = comment();
                system["id"] = json!(11);
                system["system"] = json!(true);
                json!([comment(), system])
            },
        ),
    ]);
    let client = GitLabClient::new().unwrap();
    let config = [("serverUrl".into(), api.url.clone())]
        .into_iter()
        .collect();
    let token = ProviderToken::new("token".into());
    let items = client
        .list_issues(&config, &token, &relevance_repository())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "101");
    assert_eq!(items[0].number, 7);
    assert_eq!(items[0].labels, vec!["bug"]);
    assert_eq!(items[0].assignees, vec!["bob"]);
    assert_eq!(
        items[0].relationships.subscribers.scope.as_deref(),
        Some("1")
    );
    assert_eq!(
        items[0].relationships.evaluate("1").reasons,
        vec![
            RelevanceReason::Authored,
            RelevanceReason::Subscribed,
            RelevanceReason::Discussed
        ]
    );
    let details = client
        .issue_details(&config, &token, &relevance_repository(), 7)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(details.body.as_deref(), Some("Issue description"));
    assert_eq!(details.milestone.as_deref(), Some("Next"));
    assert_eq!(details.comments.len(), 1);
    assert_eq!(details.comments[0].body, "A discussion comment");
    assert!(
        details
            .issue
            .relationships
            .evaluate("1")
            .reasons
            .contains(&RelevanceReason::Discussed)
    );
    assert!(details.issue.relationships.participants.complete);
    api.finish();
}

#[tokio::test]
async fn disabled_tracker_is_unsupported_and_makes_no_issue_requests() {
    let api = MockApi::start(vec![Exchange::json(
        "/api/v4/projects/3",
        AUTH,
        json!({"issues_access_level":"disabled"}),
    )]);
    let client = GitLabClient::new().unwrap();
    let config = [("serverUrl".into(), api.url.clone())]
        .into_iter()
        .collect();
    assert_eq!(
        client
            .list_issues(
                &config,
                &ProviderToken::new("token".into()),
                &relevance_repository()
            )
            .await,
        Ok(None)
    );
    api.finish();
}

#[tokio::test]
async fn pagination_failure_is_an_error_not_an_empty_or_partial_snapshot() {
    let mut first = Exchange::json(
        "/api/v4/projects/3/issues?state=opened&scope=all&order_by=updated_at&sort=desc&per_page=50&page=1",
        AUTH,
        json!([issue()]),
    );
    first.headers.push_str("X-Next-Page: 2\r\n");
    let mut last = Exchange::denied(
        "/api/v4/projects/3/issues?state=opened&scope=all&order_by=updated_at&sort=desc&per_page=50&page=2",
        AUTH,
    );
    last.status = 429;
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v4/projects/3",
            AUTH,
            json!({"issues_access_level":"enabled"}),
        ),
        first,
        last,
    ]);
    let client = GitLabClient::new().unwrap();
    let config = [("serverUrl".into(), api.url.clone())]
        .into_iter()
        .collect();
    assert_eq!(
        client
            .list_issues(
                &config,
                &ProviderToken::new("token".into()),
                &relevance_repository()
            )
            .await,
        Err(Failure::RateLimited)
    );
    api.finish();
}

#[tokio::test]
async fn unavailable_issue_is_none_but_permission_failures_remain_errors() {
    for status in [404, 403, 401] {
        let mut response = Exchange::denied("/api/v4/projects/3/issues/7", AUTH);
        response.status = status;
        let api = MockApi::start(vec![
            Exchange::json(
                "/api/v4/projects/3",
                AUTH,
                json!({"issues_access_level":"enabled"}),
            ),
            response,
        ]);
        let client = GitLabClient::new().unwrap();
        let config = [("serverUrl".into(), api.url.clone())]
            .into_iter()
            .collect();
        let result = client
            .issue_details(
                &config,
                &ProviderToken::new("token".into()),
                &relevance_repository(),
                7,
            )
            .await;
        assert_eq!(
            result,
            match status {
                404 => Ok(None),
                401 => Err(Failure::InvalidCredentials),
                _ => Err(Failure::PermissionDenied),
            }
        );
        api.finish();
    }
}

#[test]
fn lifecycle_unknown_states_and_deleted_authors_are_handled_conservatively() {
    for (state, expected) in [
        ("opened", Ok(IssueState::Open)),
        ("closed", Ok(IssueState::Closed)),
        ("future", Err(Failure::UnexpectedResponse)),
    ] {
        let mut value = issue();
        value["state"] = json!(state);
        value["author"] = Value::Null;
        let parsed: ProviderIssue = serde_json::from_value(value).unwrap();
        assert_eq!(parsed.summary().map(|issue| issue.state), expected);
    }
}

#[test]
fn personal_issues_match_authorship_subscription_and_non_system_discussion_not_assignment() {
    let mut value = issue();
    value["subscribed"] = json!(false);
    let parsed: ProviderIssue = serde_json::from_value(value).unwrap();
    let mut discussed = comment();
    discussed["author"] = json!({"id":2,"username":"bob"});
    let notes: Vec<Note> = serde_json::from_value(json!([discussed.clone()])).unwrap();
    assert!(
        !parsed
            .relationships(Some(2), Some(&[]))
            .evaluate("2")
            .matches()
    );
    assert_eq!(
        parsed
            .relationships(Some(2), Some(&notes))
            .evaluate("2")
            .reasons,
        vec![RelevanceReason::Discussed]
    );
    discussed["system"] = json!(true);
    let notes: Vec<Note> = serde_json::from_value(json!([discussed])).unwrap();
    assert!(
        !parsed
            .relationships(Some(2), Some(&notes))
            .evaluate("2")
            .matches()
    );
    assert!(
        parsed
            .relationships(None, Some(&notes))
            .evaluate("1")
            .matches()
    );
    assert!(!parsed.relationships(Some(2), None).participants.complete);
}

#[tokio::test]
async fn optional_subscription_and_discussion_failures_do_not_break_issue_listing() {
    let mut value = issue();
    value.as_object_mut().unwrap().remove("subscribed");
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v4/projects/3",
            AUTH,
            json!({"issues_access_level":"enabled"}),
        ),
        Exchange::json(
            "/api/v4/projects/3/issues?state=opened&scope=all&order_by=updated_at&sort=desc&per_page=50&page=1",
            AUTH,
            json!([value]),
        ),
        Exchange::json("/api/v4/user", AUTH, json!({"id":2})),
        Exchange::denied("/api/v4/projects/3/issues/7", AUTH),
        Exchange::denied(
            "/api/v4/projects/3/issues/7/notes?order_by=created_at&sort=asc&per_page=50&page=1",
            AUTH,
        ),
    ]);
    let client = GitLabClient::new().unwrap();
    let config = [("serverUrl".into(), api.url.clone())]
        .into_iter()
        .collect();
    let issues = client
        .list_issues(
            &config,
            &ProviderToken::new("token".into()),
            &relevance_repository(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(issues.len(), 1);
    assert!(!issues[0].relationships.evaluate("2").matches());
    assert!(!issues[0].relationships.participants.complete);
    api.finish();
}
