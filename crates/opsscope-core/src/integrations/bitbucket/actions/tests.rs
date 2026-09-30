use super::*;

#[tokio::test]
async fn merge_checks_block_conflicts_unknown_state_and_partial_results() {
    for checks in [
        json!({"values":[]}),
        json!({"values":[{"type":"git_mergeability_check","status":"FAILED","blocking":true}]}),
        json!({"values":[{"type":"git_mergeability_check","status":"PASSED","blocking":false},{"type":"standard_merge_check","status":"FAILED","blocking":true}]}),
        json!({"values":[{"type":"git_mergeability_check","status":"PASSED","blocking":false}],"next":"https://example.com/next"}),
        json!({"values":[{"type":"git_mergeability_check","status":"PASSED","blocking":false}],"size":2}),
    ] {
        let auth = "authorization: Basic dXNlckBleGFtcGxlLmNvbTpzZWNyZXQ=";
        let api = MockApi::start(vec![
            Exchange::json(
                "/2.0/repositories/team/app/pullrequests/7",
                auth,
                json!({"state":"OPEN","draft":false,"source":{"commit":{"hash":"abc"}},"destination":{"commit":{"hash":"def"}}}),
            ),
            Exchange::json(
                "/2.0/repositories/team/app/pullrequests/7/mergeability/checks",
                auth,
                checks,
            ),
        ]);
        let mut client = BitbucketClient::new().unwrap();
        client.api_base = Url::parse(&format!("{}/2.0/", api.url)).unwrap();
        let config = [
            (WORKSPACE_KEY.into(), "team".into()),
            (EMAIL_KEY.into(), "user@example.com".into()),
        ]
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
        assert!(options.actions[0].disabled_reason.is_some());
        api.finish();
    }
}

#[tokio::test]
async fn bitbucket_merge_keeps_source_branch_and_accepts_async_results() {
    for status in [200, 202] {
        for (configured, expected) in [(None, "squash"), (Some("merge_commit"), "merge_commit")] {
            let auth = "authorization: Basic dXNlckBleGFtcGxlLmNvbTpzZWNyZXQ=";
            let mut mutation = Exchange::json(
                "/2.0/repositories/team/app/pullrequests/7/merge",
                auth,
                if status == 200 {
                    json!({"state":"MERGED"})
                } else {
                    json!({"task_id":"pending"})
                },
            );
            mutation.method = "POST".into();
            mutation.status = status;
            mutation.request_body = Some(
                json!({"type":"pullrequest","merge_strategy":expected,"close_source_branch":false}),
            );
            let api = MockApi::start(vec![
                Exchange::json(
                    "/2.0/repositories/team/app/pullrequests/7",
                    auth,
                    json!({"state":"OPEN","draft":false,"source":{"commit":{"hash":"abc"}},"destination":{"commit":{"hash":"def"}}}),
                ),
                Exchange::json(
                    "/2.0/repositories/team/app/pullrequests/7/mergeability/checks",
                    auth,
                    json!({"size":1,"values":[{"type":"git_mergeability_check","status":"PASSED","blocking":false}]}),
                ),
                Exchange::json(
                    "/2.0/repositories/team/app/pullrequests/7",
                    auth,
                    json!({"state":"OPEN","draft":false,"source":{"commit":{"hash":"abc"}},"destination":{"commit":{"hash":"def"},"branch":{"name":"main"}}}),
                ),
                Exchange::json(
                    "/2.0/repositories/team/app/refs/branches/main",
                    auth,
                    json!({"default_merge_strategy": configured}),
                ),
                mutation,
            ]);
            let mut client = BitbucketClient::new().unwrap();
            client.api_base = Url::parse(&format!("{}/2.0/", api.url)).unwrap();
            let config = [
                (WORKSPACE_KEY.into(), "team".into()),
                (EMAIL_KEY.into(), "user@example.com".into()),
            ]
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
            assert_eq!(options.revision.as_deref(), Some("abc:def"));
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
}
use crate::integrations::test_support::{Exchange, MockApi, relevance_repository};

#[test]
fn replay_preserves_commit_branch_and_selector_but_rejects_unknown_targets() {
    let target = json!({"type":"pipeline_ref_target","ref_name":"main","ref_type":"branch","commit":{"hash":"abc"},"selector":{"type":"custom","pattern":"build"},"untrusted":"discard"});
    let result = repeat_target(&target).unwrap();
    assert_eq!(result["commit"]["hash"], "abc");
    assert_eq!(result["ref_name"], "main");
    assert_eq!(result["selector"], target["selector"]);
    assert!(result.get("untrusted").is_none());
    let pr = json!({"type":"pipeline_pullrequest_target","source":"feature","destination":"main","commit":{"hash":"head"},"destination_commit":{"hash":"base"},"pullrequest":{"id":7},"selector":{"type":"pull-requests","pattern":"**"}});
    let repeated = repeat_target(&pr).unwrap();
    assert_eq!(repeated["destination_commit"]["hash"], "base");
    assert_eq!(repeated["pullrequest"]["id"], 7);
    assert_eq!(repeated["source"], "feature");
    assert!(
        repeat_target(&json!({"type":"pipeline_pullrequest_target","commit":{"hash":"abc"}}))
            .is_none()
    );
    assert!(repeat_target(&json!({"type":"pipeline_ref_target"})).is_none());
}

#[tokio::test]
async fn creates_pipeline_at_original_commit_without_copying_variables() {
    let auth = "authorization: Basic dXNlckBleGFtcGxlLmNvbTpzZWNyZXQ=";
    let target = json!({"type":"pipeline_ref_target","ref_name":"main","ref_type":"branch","commit":{"hash":"abc"}});
    let pipeline = json!({"state":{"name":"COMPLETED"},"target":target,"completed_on":"today"});
    let mut mutation = Exchange::json(
        "/2.0/repositories/team/app/pipelines",
        auth,
        json!({"uuid":"new-run-uuid"}),
    );
    mutation.method = "POST".into();
    mutation.request_body = Some(
        json!({"target":{"type":"pipeline_ref_target","ref_name":"main","ref_type":"branch","commit":{"type":"commit","hash":"abc"}}}),
    );
    let api = MockApi::start(vec![
        Exchange::json(
            "/2.0/repositories/team/app/pipelines/run-uuid",
            auth,
            pipeline.clone(),
        ),
        Exchange::json(
            "/2.0/repositories/team/app/pipelines/run-uuid",
            auth,
            pipeline,
        ),
        mutation,
    ]);
    let mut client = BitbucketClient::new().unwrap();
    client.api_base = Url::parse(&format!("{}/2.0/", api.url)).unwrap();
    let config = [
        (WORKSPACE_KEY.into(), "team".into()),
        (EMAIL_KEY.into(), "user@example.com".into()),
    ]
    .into_iter()
    .collect();
    let token = ProviderToken::new("secret".into());
    let repo = relevance_repository();
    let target = ActionTarget::WorkflowRun {
        run_id: "run-uuid".into(),
    };
    let options = client
        .actions(&config, &token, &repo, &target)
        .await
        .unwrap();
    assert!(options.actions[0].disabled_reason.is_none());
    let refreshed_target = client
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
    assert_eq!(
        refreshed_target,
        Some(ActionTarget::WorkflowRun {
            run_id: "new-run-uuid".into()
        })
    );
    assert_eq!(
        client
            .perform_action(
                &config,
                &token,
                &repo,
                &ActionTarget::ChangeRequest {
                    number: 1
                },
                SourceAction::UpdateBranch,
                None
            )
            .await,
        Err(ActionFailure::Unsupported)
    );
    api.finish();
}
