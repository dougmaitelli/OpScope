use super::*;
#[tokio::test]
async fn merge_uses_revision_guard_and_does_not_request_bypass_or_deletion() {
    let auth = "private-token: secret";
    let mut mutation = Exchange::json(
        "/api/v4/projects/3/merge_requests/7/merge",
        auth,
        json!({"state":"merged"}),
    );
    mutation.method = "PUT".into();
    mutation.request_body =
        Some(json!({"sha":"abc","auto_merge":false,"should_remove_source_branch":false}));
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v4/projects/3/merge_requests/7?include_rebase_in_progress=true",
            auth,
            json!({"state":"opened","sha":"abc","has_conflicts":false,"detailed_merge_status":"mergeable","draft":false,"rebase_in_progress":false}),
        ),
        mutation,
    ]);
    let client = GitLabClient::new().unwrap();
    let config = [(SERVER_URL_KEY.into(), api.url.clone())]
        .into_iter()
        .collect();
    let token = ProviderToken::new("secret".into());
    let repo = relevance_repository();
    let target = ActionTarget::ChangeRequest {
        number: 7,
    };
    let options = client
        .actions(&config, &token, &repo, &target)
        .await
        .unwrap();
    assert!(
        options
            .actions
            .iter()
            .any(|a| a.action == SourceAction::MergeChangeRequest && a.disabled_reason.is_none())
    );
    client
        .perform_action(
            &config,
            &token,
            &repo,
            &target,
            SourceAction::MergeChangeRequest,
            options.revision.as_deref(),
        )
        .await
        .unwrap();
    api.finish();
}

#[tokio::test]
async fn merge_is_disabled_for_drafts_and_unknown_draft_state() {
    for draft in [json!(true), json!(null)] {
        let mut pull = json!({"state":"opened","sha":"abc","has_conflicts":false,"detailed_merge_status":"mergeable","draft":false,"rebase_in_progress":false});
        pull["draft"] = draft;
        let api = MockApi::start(vec![Exchange::json(
            "/api/v4/projects/3/merge_requests/7?include_rebase_in_progress=true",
            "private-token: secret",
            pull,
        )]);
        let client = GitLabClient::new().unwrap();
        let config = [(SERVER_URL_KEY.into(), api.url.clone())]
            .into_iter()
            .collect();
        let options = client
            .actions(
                &config,
                &ProviderToken::new("secret".into()),
                &relevance_repository(),
                &ActionTarget::ChangeRequest {
                    number: 7,
                },
            )
            .await
            .unwrap();
        assert!(
            options.actions.iter().any(
                |a| a.action == SourceAction::MergeChangeRequest && a.disabled_reason.is_some()
            )
        );
        api.finish();
    }
}
use crate::integrations::test_support::{Exchange, MockApi, relevance_repository};
use serde_json::json;

#[tokio::test]
async fn workflow_action_uses_authenticated_provider_endpoint() {
    let auth = "private-token: secret";
    let mut mutation = Exchange::json("/api/v4/projects/3/pipelines/7/retry", auth, json!({}));
    mutation.method = "POST".into();
    mutation.status = 204;
    mutation.body.clear();
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v4/projects/3/pipelines/7",
            auth,
            json!({"status":"failed","updated_at":"today"}),
        ),
        mutation,
    ]);
    let client = GitLabClient::new().unwrap();
    let config = [(SERVER_URL_KEY.into(), api.url.clone())]
        .into_iter()
        .collect();
    let token = ProviderToken::new("secret".into());
    let repo = relevance_repository();
    let target = ActionTarget::WorkflowRun {
        run_id: "7".into(),
    };
    let options = client
        .actions(&config, &token, &repo, &target)
        .await
        .unwrap();
    assert!(options.actions[0].disabled_reason.is_none());
    assert_eq!(options.revision.as_deref(), Some("today"));
    client
        .perform_action(
            &config,
            &token,
            &repo,
            &target,
            SourceAction::RerunWorkflow,
            options.revision.as_deref(),
        )
        .await
        .unwrap();
    api.finish();
}

#[tokio::test]
async fn branch_update_uses_update_endpoint_not_merge_endpoint() {
    let auth = "private-token: secret";
    let mut mutation = Exchange::json(
        "/api/v4/projects/3/merge_requests/7/rebase",
        auth,
        json!({}),
    );
    mutation.method = "PUT".into();

    mutation.status = 202;
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v4/projects/3/merge_requests/7?include_rebase_in_progress=true",
            auth,
            json!({"state":"opened","sha":"abc","has_conflicts":false,"detailed_merge_status":"mergeable","rebase_in_progress":false}),
        ),
        mutation,
    ]);
    let client = GitLabClient::new().unwrap();
    let config = [(SERVER_URL_KEY.into(), api.url.clone())]
        .into_iter()
        .collect();
    let token = ProviderToken::new("secret".into());
    let repo = relevance_repository();
    let target = ActionTarget::ChangeRequest {
        number: 7,
    };
    let options = client
        .actions(&config, &token, &repo, &target)
        .await
        .unwrap();
    assert!(options.actions[0].disabled_reason.is_none());
    client
        .perform_action(
            &config,
            &token,
            &repo,
            &target,
            SourceAction::UpdateBranch,
            options.revision.as_deref(),
        )
        .await
        .unwrap();
    api.finish();
}
