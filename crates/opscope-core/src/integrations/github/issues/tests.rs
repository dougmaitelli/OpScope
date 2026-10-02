use super::*;
use crate::application::SourceModule;
use crate::integrations::test_support::{Exchange, MockApi, relevance_repository};
use serde_json::json;

#[tokio::test]
async fn issue_operations_send_the_feature_queries_through_shared_transport() {
    let repo = relevance_repository();
    let auth = "authorization: Bearer test-token";
    let api = MockApi::start(vec![
        Exchange::graphql(
            "/api/graphql",
            auth,
            json!({"owner":"team","name":"app","first":100,"after":null}),
            &[
                "query OpenIssues",
                "states: OPEN",
                "comments { totalCount }",
            ],
            json!({"data":{"repository":{"issues":{"nodes":[],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}}),
        ),
        Exchange::graphql(
            "/api/graphql",
            auth,
            json!({"owner":"team","name":"app","number":7}),
            &[
                "query IssueDetails",
                "comments(last: 100)",
                "milestone { title }",
            ],
            json!({"data":{"repository":{"issue":null}}}),
        ),
    ]);
    let client = GitHubClient::new().unwrap();
    let config = [("serverUrl".into(), api.url.clone())]
        .into_iter()
        .collect();
    let token = ProviderToken::new("test-token".into());
    assert_eq!(
        client.list_issues(&config, &token, &repo).await.unwrap(),
        Some(vec![])
    );
    assert_eq!(
        client
            .issue_details(&config, &token, &repo, 7)
            .await
            .unwrap(),
        None
    );
    api.finish();
}

#[test]
fn response_maps_issue_metadata_and_comments() -> Result<(), Box<dyn std::error::Error>> {
    let issue: GitHubIssue = serde_json::from_str(
        r#"{
          "id":"I_kwDOExample","number":7,"title":"Improve diagnostics",
          "author":{"login":"octocat"},"state":"OPEN",
          "labels":{"nodes":[{"name":"bug"}]},
          "assignees":{"nodes":[{"login":"maintainer"}]},
          "comments":{"totalCount":1,"nodes":[{
            "id":"IC_example","author":{"login":"reviewer"},"body":"Confirmed",
            "createdAt":"2026-09-26T18:00:00Z","updatedAt":"2026-09-26T18:00:00Z"
          }]},
          "createdAt":"2026-09-25T18:00:00Z","updatedAt":"2026-09-26T18:00:00Z",
          "url":"https://github.com/octocat/Hello-World/issues/7",
          "body":"Details","milestone":{"title":"v1"}
        }"#,
    )?;
    let details = issue.into_details();
    assert_eq!(details.issue.labels, ["bug"]);
    assert_eq!(details.issue.assignees, ["maintainer"]);
    assert_eq!(details.issue.comment_count, 1);
    assert_eq!(details.milestone.as_deref(), Some("v1"));
    assert_eq!(details.comments[0].body, "Confirmed");
    Ok(())
}
