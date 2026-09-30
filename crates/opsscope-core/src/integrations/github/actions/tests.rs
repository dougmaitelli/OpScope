use super::*;

#[tokio::test]
async fn core_rejects_merge_when_state_or_revision_is_unsafe() {
    for (state, mergeable, draft, status, revision) in [
        ("closed", Some(true), false, "clean", "abc"),
        ("open", Some(false), false, "dirty", "abc"),
        ("open", None, false, "unknown", "abc"),
        ("open", Some(true), true, "clean", "abc"),
        ("open", Some(true), false, "blocked", "abc"),
        ("open", Some(true), false, "unstable", "abc"),
        ("open", Some(true), false, "clean", "old-head"),
    ] {
        let api = MockApi::start(vec![Exchange::json(
            "/api/v3/repos/team/app/pulls/7",
            "authorization: Bearer secret",
            json!({"state":state,"merged":false,"mergeable":mergeable,"draft":draft,"mergeable_state":status,"head":{"sha":"abc"},"user":{"login":"alice","type":"User"}}),
        )]);
        assert_eq!(
            action_service(&api)
                .execute(
                    "connection",
                    "3",
                    &ActionTarget::ChangeRequest {
                        number: 7
                    },
                    SourceAction::MergeChangeRequest,
                    Some(revision)
                )
                .await,
            Err(ActionFailure::Conflict)
        );
        api.finish();
    }
}

#[tokio::test]
async fn a_success_status_with_merged_false_is_not_reported_as_a_merge() {
    let mut exchange = Exchange::json(
        "/api/v3/repos/team/app/pulls/7/merge",
        "authorization: Bearer secret",
        json!({"merged":false}),
    );
    exchange.method = "PUT".into();
    exchange.request_body = Some(json!({"sha":"abc","merge_method":"merge"}));
    let api = MockApi::start(vec![exchange]);
    let client = GitHubClient::new().unwrap();
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
        Err(ActionFailure::Conflict)
    );
    api.finish();
}
#[tokio::test]
async fn merge_uses_revision_guard_and_does_not_request_bypass_or_deletion() {
    let auth = "authorization: Bearer secret";
    let mut mutation = Exchange::json(
        "/api/v3/repos/team/app/pulls/7/merge",
        auth,
        json!({"merged":true}),
    );
    mutation.method = "PUT".into();
    mutation.request_body = Some(json!({"sha":"abc","merge_method":"merge"}));
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v3/repos/team/app/pulls/7",
            auth,
            json!({"state":"open","merged":false,"mergeable":true,"mergeable_state":"clean","draft":false,"head":{"sha":"abc"},"user":{"login":"alice","type":"User"}}),
        ),
        mutation,
    ]);
    let client = GitHubClient::new().unwrap();
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
        let mut pull = json!({"state":"open","merged":false,"mergeable":true,"mergeable_state":"clean","draft":false,"head":{"sha":"abc"},"user":{"login":"alice","type":"User"}});
        pull["draft"] = draft;
        let api = MockApi::start(vec![Exchange::json(
            "/api/v3/repos/team/app/pulls/7",
            "authorization: Bearer secret",
            pull,
        )]);
        let client = GitHubClient::new().unwrap();
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

fn action_service(api: &MockApi) -> crate::application::SourceActions {
    use crate::application::*;
    use crate::persistence::{EncryptedSecretStore, ServerMasterKey, SqliteDatabase};
    use crate::source_data::{
        ReadThroughSourceData, RepositorySnapshot, SourceDataCache, SourceDataCachePolicy,
    };
    use std::sync::Arc;
    let db = SqliteDatabase::in_memory().unwrap();
    let reference = SecretReference::for_connection("connection", "test");
    let secrets = EncryptedSecretStore::new(db.clone(), ServerMasterKey::generate().unwrap());
    secrets
        .store(&reference, &ProviderToken::new("secret".into()))
        .unwrap();
    db.save(&StoredConnection {
        id: "connection".into(),
        source_id: "github".into(),
        unique_key: api.url.clone(),
        label: "Test".into(),
        configuration: [(SERVER_URL_KEY.into(), api.url.clone())]
            .into_iter()
            .collect(),
        account: ValidatedAccount {
            external_id: "account".into(),
            name: "Alice".into(),
            handle: None,
            profile_url: None,
        },
        secret_reference: reference,
    })
    .unwrap();
    db.replace_repositories(
        "connection",
        "account",
        &RepositorySnapshot {
            refreshed_at: 0,
            repositories: vec![relevance_repository()],
        },
    )
    .unwrap();
    let registry = SourceRegistry::new(vec![Arc::new(GitHubClient::new().unwrap())]);
    let db = Arc::new(db);
    let secrets = Arc::new(secrets);
    let data = Arc::new(ReadThroughSourceData::cached(
        registry.clone(),
        db.clone(),
        secrets.clone(),
        db.clone(),
        SourceDataCachePolicy::default(),
    ));
    SourceActions::new(registry, db, secrets, data)
}

#[tokio::test]
async fn core_rechecks_conflicts_closed_prs_unknown_mergeability_and_stale_revisions_without_writing()
 {
    for (state, mergeable, revision) in [
        ("open", Some(false), "abc"),
        ("open", None, "abc"),
        ("closed", Some(true), "abc"),
        ("open", Some(true), "old"),
    ] {
        let api = MockApi::start(vec![Exchange::json(
            "/api/v3/repos/team/app/pulls/7",
            "authorization: Bearer secret",
            json!({"state":state,"merged":false,"mergeable":mergeable,"head":{"sha":"abc"},"user":{"login":"alice","type":"User"}}),
        )]);
        assert_eq!(
            action_service(&api)
                .execute(
                    "connection",
                    "3",
                    &ActionTarget::ChangeRequest {
                        number: 7
                    },
                    SourceAction::UpdateBranch,
                    Some(revision)
                )
                .await,
            Err(ActionFailure::Conflict)
        );
        api.finish();
    }
}

#[tokio::test]
async fn dependabot_commands_require_bot_identity_and_use_only_allowlisted_comments() {
    for (kind, command) in [
        ("User", SourceAction::DependabotRebase),
        ("Bot", SourceAction::DependabotRebase),
        ("Bot", SourceAction::DependabotRecreate),
    ] {
        let auth = "authorization: Bearer secret";
        let mut exchanges = vec![Exchange::json(
            "/api/v3/repos/team/app/pulls/7",
            auth,
            json!({"state":"open","merged":false,"mergeable":false,"head":{"sha":"abc"},"user":{"login":"dependabot","type":kind}}),
        )];
        if kind == "Bot" {
            let mut write =
                Exchange::json("/api/v3/repos/team/app/issues/7/comments", auth, json!({}));
            write.method = "POST".into();
            write.request_body = Some(
                json!({"body": if command == SourceAction::DependabotRebase { "@dependabot rebase" } else { "@dependabot recreate" }}),
            );
            exchanges.push(write);
        }
        let api = MockApi::start(exchanges);
        let client = GitHubClient::new().unwrap();
        let config = [(SERVER_URL_KEY.into(), api.url.clone())]
            .into_iter()
            .collect();
        let token = ProviderToken::new("secret".into());
        let repo = relevance_repository();
        let target = ActionTarget::ChangeRequest {
            number: 7,
        };
        if kind == "User" {
            assert_eq!(
                action_service(&api)
                    .execute("connection", "3", &target, command, Some("abc"))
                    .await,
                Err(ActionFailure::Unsupported)
            );
        } else {
            let options = client
                .actions(&config, &token, &repo, &target)
                .await
                .unwrap();
            assert!(
                options
                    .actions
                    .iter()
                    .any(|option| option.action == command && option.disabled_reason.is_none())
            );
            client
                .perform_action(&config, &token, &repo, &target, command, Some("abc"))
                .await
                .unwrap();
        }
        api.finish();
    }
}

#[tokio::test]
async fn write_failures_are_classified_without_retrying() {
    for (status, failure) in [
        (
            403,
            ActionFailure::Source(ConnectionValidationFailure::PermissionDenied),
        ),
        (409, ActionFailure::Conflict),
        (422, ActionFailure::Conflict),
        (
            429,
            ActionFailure::Source(ConnectionValidationFailure::RateLimited),
        ),
        (500, ActionFailure::OutcomeUnknown),
    ] {
        let mut mutation = Exchange::json(
            "/api/v3/repos/team/app/actions/runs/7/rerun",
            "authorization: Bearer secret",
            json!({}),
        );
        mutation.method = "POST".into();
        mutation.status = status;
        let api = MockApi::start(vec![mutation]);
        let client = GitHubClient::new().unwrap();
        let config = [(SERVER_URL_KEY.into(), api.url.clone())]
            .into_iter()
            .collect();
        assert_eq!(
            client
                .perform_action(
                    &config,
                    &ProviderToken::new("secret".into()),
                    &relevance_repository(),
                    &ActionTarget::WorkflowRun {
                        run_id: "7".into()
                    },
                    SourceAction::RerunWorkflow,
                    Some("1")
                )
                .await,
            Err(failure)
        );
        api.finish();
    }
}

#[tokio::test]
async fn accepted_action_remains_successful_when_cache_refresh_fails() {
    let auth = "authorization: Bearer secret";
    let mut mutation = Exchange::json(
        "/api/v3/repos/team/app/actions/runs/7/rerun",
        auth,
        json!({}),
    );
    mutation.method = "POST".into();
    mutation.status = 201;
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v3/repos/team/app/actions/runs/7",
            auth,
            json!({"status":"completed","run_attempt":1}),
        ),
        mutation,
        Exchange::denied(
            "/api/v3/repos/team/app/actions/runs?per_page=100&page=1",
            auth,
        ),
    ]);
    assert_eq!(
        action_service(&api)
            .execute(
                "connection",
                "3",
                &ActionTarget::WorkflowRun {
                    run_id: "7".into()
                },
                SourceAction::RerunWorkflow,
                Some("1")
            )
            .await,
        Ok(())
    );
    api.finish();
}

#[tokio::test]
async fn workflow_action_uses_authenticated_provider_endpoint() {
    let auth = "authorization: Bearer secret";
    let mut mutation = Exchange::json(
        "/api/v3/repos/team/app/actions/runs/7/rerun",
        auth,
        json!({}),
    );
    mutation.method = "POST".into();
    mutation.status = 204;
    mutation.body.clear();
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v3/repos/team/app/actions/runs/7",
            auth,
            json!({"status":"completed","run_attempt":2}),
        ),
        mutation,
    ]);
    let client = GitHubClient::new().unwrap();
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
    let auth = "authorization: Bearer secret";
    let mut mutation = Exchange::json(
        "/api/v3/repos/team/app/pulls/7/update-branch",
        auth,
        json!({}),
    );
    mutation.method = "PUT".into();
    mutation.request_body = Some(json!({"expected_head_sha":"abc"}));
    mutation.status = 202;
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v3/repos/team/app/pulls/7",
            auth,
            json!({"state":"open","merged":false,"mergeable":true,"head":{"sha":"abc"},"user":{"login":"alice","type":"User"}}),
        ),
        mutation,
    ]);
    let client = GitHubClient::new().unwrap();
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
