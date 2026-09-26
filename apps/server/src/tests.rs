use super::router;
use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use ciwatcher_core::application::{
    ConnectionValidationFailure, CredentialField, ProviderToken, SourceDescriptor, SourceModule,
    SourceRegistry, ValidatedAccount,
};
use ciwatcher_core::contracts::{
    CONNECTIONS_HTTP_PATH, HEALTH_HTTP_PATH, ListRepositoriesResponse, ListSourcesResponse,
    ListWorkflowsResponse, REPOSITORIES_HTTP_PATH, REPOSITORY_SELECTIONS_HTTP_PATH,
    SOURCES_HTTP_PATH, WORKFLOWS_HTTP_PATH,
};
use ciwatcher_core::domain::{
    Repository, RepositoryVisibility, RunLifecycle, RunOutcome, Workflow, WorkflowRun,
    WorkflowState,
};
use ciwatcher_core::persistence::{EncryptedSecretStore, ServerMasterKey, SqliteDatabase};
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt;

#[derive(Clone)]
struct TestSourceModule;

#[async_trait]
impl SourceModule for TestSourceModule {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: "example".to_owned(),
            name: "Example".to_owned(),
            description: "Example monitoring source".to_owned(),
            abbreviation: "EX".to_owned(),
            credential: CredentialField {
                label: "Access token".to_owned(),
                placeholder: "token_…".to_owned(),
                help: "Use a read-only token.".to_owned(),
            },
        }
    }

    async fn validate(
        &self,
        token: &ProviderToken,
    ) -> Result<ValidatedAccount, ConnectionValidationFailure> {
        assert_eq!(token.expose(), "test_credential");
        Ok(ValidatedAccount {
            external_id: "42".to_owned(),
            name: "Example Account".to_owned(),
            handle: Some("example-user".to_owned()),
            profile_url: Some("https://example.com/account".to_owned()),
        })
    }

    async fn list_repositories(
        &self,
        token: &ProviderToken,
    ) -> Result<Vec<Repository>, ConnectionValidationFailure> {
        assert_eq!(token.expose(), "test_credential");
        Ok(vec![Repository {
            id: "repository-1".to_owned(),
            owner: "example-user".to_owned(),
            name: "example-project".to_owned(),
            description: Some("Example repository".to_owned()),
            visibility: RepositoryVisibility::Private,
            web_url: "https://example.com/example-user/example-project".to_owned(),
        }])
    }

    async fn list_workflows(
        &self,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Vec<Workflow>, ConnectionValidationFailure> {
        assert_eq!(token.expose(), "test_credential");
        assert_eq!(repository.id, "repository-1");
        Ok(vec![Workflow {
            id: "workflow-1".to_owned(),
            name: "Build".to_owned(),
            path: ".github/workflows/build.yml".to_owned(),
            state: WorkflowState::Active,
            web_url: "https://example.com/example-user/example-project/actions/workflows/build.yml"
                .to_owned(),
        }])
    }

    async fn list_workflow_runs(
        &self,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Vec<WorkflowRun>, ConnectionValidationFailure> {
        assert_eq!(token.expose(), "test_credential");
        assert_eq!(repository.id, "repository-1");
        Ok(vec![WorkflowRun {
            id: "run-1".to_owned(),
            workflow_id: "workflow-1".to_owned(),
            run_number: 12,
            attempt: 1,
            title: "Build main".to_owned(),
            lifecycle: RunLifecycle::Completed,
            outcome: RunOutcome::Success,
            branch: Some("main".to_owned()),
            commit_sha: "abcdef123456".to_owned(),
            actor: Some("example-user".to_owned()),
            trigger: "push".to_owned(),
            created_at: "2026-09-26T18:00:00Z".to_owned(),
            started_at: Some("2026-09-26T18:00:02Z".to_owned()),
            updated_at: "2026-09-26T18:03:00Z".to_owned(),
            web_url: "https://example.com/runs/1".to_owned(),
            provider_status: "completed".to_owned(),
            provider_conclusion: Some("success".to_owned()),
        }])
    }
}

fn test_registry() -> SourceRegistry {
    SourceRegistry::new(vec![Arc::new(TestSourceModule)])
}

#[tokio::test]
async fn both_routes_use_the_shared_contract() -> Result<(), Box<dyn std::error::Error>> {
    let database = SqliteDatabase::in_memory()?;
    let secrets = EncryptedSecretStore::new(database.clone(), ServerMasterKey::generate()?);
    let app = router(
        test_registry(),
        Arc::new(database.clone()),
        Arc::new(secrets),
        Arc::new(database.clone()),
        Arc::new(database),
    );

    let health = app
        .clone()
        .oneshot(Request::get(HEALTH_HTTP_PATH).body(Body::empty())?)
        .await?;
    assert_eq!(health.status(), StatusCode::OK);

    let workflows = app
        .oneshot(Request::get(WORKFLOWS_HTTP_PATH).body(Body::empty())?)
        .await?;
    assert_eq!(workflows.status(), StatusCode::OK);
    let body = workflows.into_body().collect().await?.to_bytes();
    let decoded: ListWorkflowsResponse = serde_json::from_slice(&body)?;
    assert_eq!(decoded.selected_repository_count, 0);
    assert!(decoded.workflows.is_empty());
    Ok(())
}

#[tokio::test]
async fn generic_source_routes_connect_list_and_disconnect_without_exposing_secrets()
-> Result<(), Box<dyn std::error::Error>> {
    let database = SqliteDatabase::in_memory()?;
    let secrets = EncryptedSecretStore::new(database.clone(), ServerMasterKey::generate()?);
    let app = router(
        test_registry(),
        Arc::new(database.clone()),
        Arc::new(secrets),
        Arc::new(database.clone()),
        Arc::new(database),
    );
    let response = app
        .clone()
        .oneshot(
            Request::post(CONNECTIONS_HTTP_PATH)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"sourceId":"example","credential":"test_credential"}"#,
                ))?,
        )
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await?.to_bytes();
    let text = std::str::from_utf8(&body)?;
    assert!(text.contains("example-user"));
    assert!(!text.contains("test_credential"));
    assert!(!text.contains("token"));

    let saved = app
        .clone()
        .oneshot(Request::get(SOURCES_HTTP_PATH).body(Body::empty())?)
        .await?;
    assert_eq!(saved.status(), StatusCode::OK);
    let body = saved.into_body().collect().await?.to_bytes();
    let decoded: ListSourcesResponse = serde_json::from_slice(&body)?;
    assert_eq!(
        decoded.sources[0]
            .connection
            .as_ref()
            .expect("saved connection")
            .handle
            .as_deref(),
        Some("example-user")
    );

    let repositories = app
        .clone()
        .oneshot(Request::get(REPOSITORIES_HTTP_PATH).body(Body::empty())?)
        .await?;
    assert_eq!(repositories.status(), StatusCode::OK);
    let body = repositories.into_body().collect().await?.to_bytes();
    let decoded: ListRepositoriesResponse = serde_json::from_slice(&body)?;
    assert_eq!(decoded.sources[0].id, "example");
    assert_eq!(decoded.sources[0].repositories[0].name, "example-project");
    assert!(!decoded.sources[0].repositories[0].selected);

    let selection = app
        .clone()
        .oneshot(
            Request::put(REPOSITORY_SELECTIONS_HTTP_PATH)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"sources":[{"sourceId":"example","repositoryIds":["repository-1"]}]}"#,
                ))?,
        )
        .await?;
    assert_eq!(selection.status(), StatusCode::OK);

    let repositories = app
        .clone()
        .oneshot(Request::get(REPOSITORIES_HTTP_PATH).body(Body::empty())?)
        .await?;
    let body = repositories.into_body().collect().await?.to_bytes();
    let decoded: ListRepositoriesResponse = serde_json::from_slice(&body)?;
    assert!(decoded.sources[0].repositories[0].selected);

    let workflows = app
        .clone()
        .oneshot(Request::get(WORKFLOWS_HTTP_PATH).body(Body::empty())?)
        .await?;
    assert_eq!(workflows.status(), StatusCode::OK);
    let body = workflows.into_body().collect().await?.to_bytes();
    let decoded: ListWorkflowsResponse = serde_json::from_slice(&body)?;
    assert_eq!(decoded.selected_repository_count, 1);
    assert_eq!(decoded.workflows.len(), 1);
    assert_eq!(decoded.workflows[0].name, "Build");
    assert_eq!(decoded.workflows[0].repository_name, "example-project");
    assert_eq!(decoded.workflows[0].runs.len(), 1);
    assert_eq!(
        decoded.workflows[0].runs[0].outcome,
        ciwatcher_core::contracts::RunOutcome::Success
    );

    let disconnected = app
        .clone()
        .oneshot(
            Request::delete(CONNECTIONS_HTTP_PATH)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"sourceId":"example"}"#))?,
        )
        .await?;
    assert_eq!(disconnected.status(), StatusCode::OK);

    let saved = app
        .oneshot(Request::get(SOURCES_HTTP_PATH).body(Body::empty())?)
        .await?;
    let body = saved.into_body().collect().await?.to_bytes();
    let decoded: ListSourcesResponse = serde_json::from_slice(&body)?;
    assert!(decoded.sources[0].connection.is_none());
    Ok(())
}
