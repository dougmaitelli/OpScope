use super::*;
#[tokio::test]
async fn failed_settings_lookup_does_not_fall_back_to_squash() {
    let api = MockApi::start(vec![Exchange::denied(
        "/api/v1/repos/team/app",
        "authorization: token secret",
    )]);
    let client = GiteaClient::new().unwrap();
    let config = [(SERVER_URL_KEY.into(), api.url.clone())]
        .into_iter()
        .collect();
    assert_eq!(
        client
            .perform_action(
                &config,
                &ProviderToken::new("secret".into()),
                &relevance_repository(),
                &ActionTarget::ChangeRequest {
                    number: 7
                },
                SourceAction::MergeChangeRequest,
                Some("abc")
            )
            .await,
        Err(ActionFailure::Source(Failure::PermissionDenied))
    );
    api.finish();
}

#[tokio::test]
async fn merge_uses_revision_guard_and_does_not_request_bypass_or_deletion() {
    for (configured, expected) in [
        (None, "squash"),
        (Some("merge"), "merge"),
        (Some("rebase"), "rebase"),
    ] {
        let auth = "authorization: token secret";
        let mut mutation = Exchange::json("/api/v1/repos/team/app/pulls/7/merge", auth, json!({}));
        mutation.method = "POST".into();
        mutation.request_body = Some(
            json!({"do":expected,"head_commit_id":"abc","force_merge":false,"merge_when_checks_succeed":false,"delete_branch_after_merge":false}),
        );
        let api = MockApi::start(vec![
            Exchange::json(
                "/api/v1/repos/team/app/pulls/7",
                auth,
                json!({"state":"open","merged":false,"mergeable":true,"draft":false,"head":{"sha":"abc"}}),
            ),
            Exchange::json(
                "/api/v1/repos/team/app",
                auth,
                json!({"default_merge_style": configured}),
            ),
            mutation,
        ]);
        let client = GiteaClient::new().unwrap();
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
            options.actions.iter().any(
                |a| a.action == SourceAction::MergeChangeRequest && a.disabled_reason.is_none()
            )
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
}

#[tokio::test]
async fn merge_is_disabled_for_drafts_and_unknown_draft_state() {
    for draft in [json!(true), json!(null)] {
        let mut pull = json!({"state":"open","merged":false,"mergeable":true,"draft":false,"head":{"sha":"abc"}});
        pull["draft"] = draft;
        let api = MockApi::start(vec![Exchange::json(
            "/api/v1/repos/team/app/pulls/7",
            "authorization: token secret",
            pull,
        )]);
        let client = GiteaClient::new().unwrap();
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
    let auth = "authorization: token secret";
    let mut mutation = Exchange::json(
        "/api/v1/repos/team/app/actions/runs/7/rerun",
        auth,
        json!({}),
    );
    mutation.method = "POST".into();
    mutation.status = 204;
    mutation.body.clear();
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v1/repos/team/app/actions/runs/7",
            auth,
            json!({"status":"completed","run_attempt":2}),
        ),
        mutation,
    ]);
    let client = GiteaClient::new().unwrap();
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
    assert_eq!(options.revision.as_deref(), Some("2"));
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
    let auth = "authorization: token secret";
    let mut mutation = Exchange::json(
        "/api/v1/repos/team/app/pulls/7/update?style=merge",
        auth,
        json!({}),
    );
    mutation.method = "POST".into();

    mutation.status = 202;
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v1/repos/team/app/pulls/7",
            auth,
            json!({"state":"open","merged":false,"mergeable":true,"head":{"sha":"abc"},"base":{"sha":"base"}}),
        ),
        Exchange::json(
            "/api/v1/repos/team/app/compare/abc...base",
            auth,
            json!({"total_commits":2}),
        ),
        mutation,
    ]);
    let client = GiteaClient::new().unwrap();
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

#[tokio::test]
async fn branch_updates_require_missing_target_commits() {
    for count in [None, Some(0), Some(2)] {
        let auth = "authorization: token secret";
        let mut exchanges = vec![Exchange::json(
            "/api/v1/repos/team/app/pulls/7",
            auth,
            json!({"state":"open","merged":false,"mergeable":true,"draft":false,
                   "head":{"sha":"abc"},"base":count.map(|_| json!({"sha":"base"}))}),
        )];
        if let Some(count) = count {
            exchanges.push(Exchange::json(
                "/api/v1/repos/team/app/compare/abc...base",
                auth,
                json!({"total_commits":count}),
            ));
        }
        let api = MockApi::start(exchanges);
        let config = [(SERVER_URL_KEY.into(), api.url.clone())]
            .into_iter()
            .collect();
        let options = GiteaClient::new()
            .unwrap()
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
        assert_eq!(
            options.actions[0].disabled_reason.is_none(),
            count.is_some_and(|n| n > 0)
        );
        assert!(options.actions[1].disabled_reason.is_none());
        api.finish();
    }
}
