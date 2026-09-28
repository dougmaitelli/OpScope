use super::*;
use reqwest::header::AUTHORIZATION;

#[test]
fn request_targets_selected_repository() -> Result<(), Box<dyn std::error::Error>> {
    let client = GitHubClient::new()?;
    let token = ProviderToken::new("github_pat_test".to_owned());
    let repository = Repository {
        id: "1".to_owned(),
        owner: "octocat".to_owned(),
        name: "Hello-World".to_owned(),
        description: None,
        visibility: crate::domain::RepositoryVisibility::Public,
        web_url: "https://github.com/octocat/Hello-World".to_owned(),
    };
    let request = request(
        &client,
        &[(
            super::super::SERVER_URL_KEY.to_owned(),
            super::super::DEFAULT_SERVER_URL.to_owned(),
        )]
        .into_iter()
        .collect(),
        &token,
        &repository,
        Some("cursor"),
    )?
    .build()?;
    let body: serde_json::Value = serde_json::from_slice(
        request
            .body()
            .and_then(reqwest::Body::as_bytes)
            .expect("JSON body"),
    )?;
    assert_eq!(body["variables"]["owner"], "octocat");
    assert_eq!(body["variables"]["after"], "cursor");
    assert_eq!(request.headers()[AUTHORIZATION], "Bearer github_pat_test");
    Ok(())
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
