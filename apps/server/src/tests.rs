use super::{ServerDependencies, router, serve_web_application};
use crate::auth::{AUTH_SESSION_PATH, CSRF_HEADER, TestSession, WebAuthentication};
use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use opsscope_core::application::{
    ConfiguredSource, ConnectionConfiguration, ConnectionValidationFailure, CredentialField,
    NoopNotificationSink, NotifyRepositoryFailures, ProviderToken, SourceDescriptor, SourceModule,
    SourceRegistry, ValidatedAccount, WorkflowRunLogsFailure,
};
use opsscope_core::contracts::{
    CONNECTIONS_HTTP_PATH, ConnectionSummary, HEALTH_HTTP_PATH, ListRepositoriesResponse,
    ListSourcesResponse, ListWorkflowsResponse, MonitoringSettingsResponse, REPOSITORIES_HTTP_PATH,
    REPOSITORY_SELECTIONS_HTTP_PATH, SETTINGS_HTTP_PATH, SOURCES_HTTP_PATH,
    SYNCHRONIZATION_HTTP_PATH, SynchronizationResponse, SynchronizationStatusResponse,
    WORKFLOW_RUN_LOGS_HTTP_PATH, WORKFLOWS_HTTP_PATH, WorkflowRunLogsResponse,
};
use opsscope_core::domain::{
    Repository, RepositoryVisibility, RunLifecycle, RunOutcome, Workflow, WorkflowRun,
    WorkflowRunLog, WorkflowRunLogs, WorkflowState,
};
use opsscope_core::persistence::{EncryptedSecretStore, ServerMasterKey, SqliteDatabase};
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;
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
            connection_fields: Vec::new(),
        }
    }

    fn configure(
        &self,
        configuration: &ConnectionConfiguration,
    ) -> Result<ConfiguredSource, ConnectionValidationFailure> {
        Ok(ConfiguredSource {
            unique_key: "example".to_owned(),
            label: "Example".to_owned(),
            configuration: configuration.clone(),
        })
    }

    async fn validate(
        &self,
        _configuration: &ConnectionConfiguration,
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
        _configuration: &ConnectionConfiguration,
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
        _configuration: &ConnectionConfiguration,
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
        _configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Vec<WorkflowRun>, ConnectionValidationFailure> {
        assert_eq!(token.expose(), "test_credential");
        assert_eq!(repository.id, "repository-1");
        Ok((1..=12)
            .map(|number| WorkflowRun {
                id: format!("run-{number}"),
                workflow_id: "workflow-1".to_owned(),
                run_number: 13 - number,
                attempt: 1,
                title: format!("Build main {number}"),
                lifecycle: RunLifecycle::Completed,
                outcome: RunOutcome::Success,
                branch: Some("main".to_owned()),
                commit_sha: format!("abcdef{number:06}"),
                actor: Some("example-user".to_owned()),
                trigger: "push".to_owned(),
                created_at: "2026-09-26T18:00:00Z".to_owned(),
                started_at: Some("2026-09-26T18:00:02Z".to_owned()),
                updated_at: "2026-09-26T18:03:00Z".to_owned(),
                web_url: format!("https://example.com/runs/{number}"),
                provider_status: "completed".to_owned(),
                provider_conclusion: Some("success".to_owned()),
            })
            .collect())
    }

    async fn workflow_run_logs(
        &self,
        _configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        run: &WorkflowRun,
    ) -> Result<WorkflowRunLogs, WorkflowRunLogsFailure> {
        assert_eq!(token.expose(), "test_credential");
        assert_eq!(repository.id, "repository-1");
        assert_eq!(run.id, "run-1");
        Ok(WorkflowRunLogs {
            files: vec![WorkflowRunLog {
                name: "build.txt".to_owned(),
                content: "Build completed".to_owned(),
            }],
            truncated: false,
        })
    }
}

fn test_registry() -> SourceRegistry {
    SourceRegistry::new(vec![Arc::new(TestSourceModule)])
}

fn test_failure_notifications(database: &SqliteDatabase) -> NotifyRepositoryFailures {
    NotifyRepositoryFailures::new(Arc::new(database.clone()), Arc::new(NoopNotificationSink))
}

fn test_dependencies(
    database: &SqliteDatabase,
    secrets: EncryptedSecretStore,
    authentication: WebAuthentication,
) -> ServerDependencies {
    ServerDependencies {
        registry: test_registry(),
        connections: Arc::new(database.clone()),
        secrets: Arc::new(secrets),
        repository_selections: Arc::new(database.clone()),
        source_data_cache: Arc::new(database.clone()),
        settings: Arc::new(database.clone()),
        failure_notifications: test_failure_notifications(database),
        authentication,
    }
}

fn authenticated(
    mut request: axum::http::request::Builder,
    session: &TestSession,
    csrf: bool,
) -> axum::http::request::Builder {
    request = request.header(header::COOKIE, &session.cookie);
    if csrf {
        request = request.header(CSRF_HEADER, &session.csrf_token);
    }
    request
}

#[tokio::test]
async fn both_routes_use_the_shared_contract() -> Result<(), Box<dyn std::error::Error>> {
    let database = SqliteDatabase::in_memory()?;
    let secrets = EncryptedSecretStore::new(database.clone(), ServerMasterKey::generate()?);
    let authentication = WebAuthentication::for_tests();
    let session = authentication.issue_test_session();
    let app = router(test_dependencies(&database, secrets, authentication));

    let health = app
        .clone()
        .oneshot(Request::get(HEALTH_HTTP_PATH).body(Body::empty())?)
        .await?;
    assert_eq!(health.status(), StatusCode::OK);

    let synchronization = app
        .clone()
        .oneshot(
            authenticated(Request::get(SYNCHRONIZATION_HTTP_PATH), &session, false)
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(synchronization.status(), StatusCode::OK);
    let body = synchronization.into_body().collect().await?.to_bytes();
    let decoded: SynchronizationStatusResponse = serde_json::from_slice(&body)?;
    assert!(!decoded.running);
    assert!(decoded.last_completed_at.is_none());

    let workflows = app
        .oneshot(
            authenticated(Request::get(WORKFLOWS_HTTP_PATH), &session, false).body(Body::empty())?,
        )
        .await?;
    assert_eq!(workflows.status(), StatusCode::OK);
    let body = workflows.into_body().collect().await?.to_bytes();
    let decoded: ListWorkflowsResponse = serde_json::from_slice(&body)?;
    assert_eq!(decoded.selected_repository_count, 0);
    assert!(decoded.workflows.is_empty());
    Ok(())
}

#[tokio::test]
async fn web_assets_use_spa_fallback_without_masking_unknown_api_routes()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    fs::write(directory.path().join("index.html"), "<main>OpsScope</main>")?;
    fs::write(directory.path().join("asset.txt"), "asset")?;
    let database = SqliteDatabase::in_memory()?;
    let secrets = EncryptedSecretStore::new(database.clone(), ServerMasterKey::generate()?);
    let app = serve_web_application(
        router(test_dependencies(
            &database,
            secrets,
            WebAuthentication::disabled_for_tests(),
        )),
        directory.path(),
    );

    for (path, expected) in [
        ("/settings", "<main>OpsScope</main>"),
        ("/asset.txt", "asset"),
    ] {
        let response = app
            .clone()
            .oneshot(Request::get(path).body(Body::empty())?)
            .await?;
        assert_eq!(response.status(), StatusCode::OK, "failed to serve {path}");
        let body = response.into_body().collect().await?.to_bytes();
        assert_eq!(body.as_ref(), expected.as_bytes());
    }

    let missing_api = app
        .oneshot(Request::get("/api/not-found").body(Body::empty())?)
        .await?;
    assert_eq!(missing_api.status(), StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn generic_source_routes_connect_list_and_disconnect_without_exposing_secrets()
-> Result<(), Box<dyn std::error::Error>> {
    let database = SqliteDatabase::in_memory()?;
    let secrets = EncryptedSecretStore::new(database.clone(), ServerMasterKey::generate()?);
    let authentication = WebAuthentication::for_tests();
    let session = authentication.issue_test_session();
    let app = router(test_dependencies(&database, secrets, authentication));
    let response = app
        .clone()
        .oneshot(
            authenticated(Request::post(CONNECTIONS_HTTP_PATH), &session, true)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"sourceId":"example","configuration":{},"credential":"test_credential"}"#,
                ))?,
        )
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await?.to_bytes();
    let text = std::str::from_utf8(&body)?;
    assert!(text.contains("example-user"));
    assert!(!text.contains("test_credential"));
    assert!(!text.contains("token"));
    let connected: ConnectionSummary = serde_json::from_slice(&body)?;
    let connection_id = connected.id;

    let duplicate = app
        .clone()
        .oneshot(
            authenticated(Request::post(CONNECTIONS_HTTP_PATH), &session, true)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"sourceId":"example","configuration":{},"credential":"test_credential"}"#,
                ))?,
        )
        .await?;
    assert_eq!(duplicate.status(), StatusCode::BAD_REQUEST);

    let saved = app
        .clone()
        .oneshot(
            authenticated(Request::get(SOURCES_HTTP_PATH), &session, false).body(Body::empty())?,
        )
        .await?;
    assert_eq!(saved.status(), StatusCode::OK);
    let body = saved.into_body().collect().await?.to_bytes();
    let decoded: ListSourcesResponse = serde_json::from_slice(&body)?;
    assert_eq!(
        decoded.sources[0].connections[0].handle.as_deref(),
        Some("example-user")
    );

    let repositories = app
        .clone()
        .oneshot(
            authenticated(Request::get(REPOSITORIES_HTTP_PATH), &session, false)
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(repositories.status(), StatusCode::OK);
    let body = repositories.into_body().collect().await?.to_bytes();
    let decoded: ListRepositoriesResponse = serde_json::from_slice(&body)?;
    assert_eq!(decoded.sources[0].id, connection_id);
    assert_eq!(decoded.sources[0].repositories[0].name, "example-project");
    assert!(!decoded.sources[0].repositories[0].selected);

    let selection = app
        .clone()
        .oneshot(
            authenticated(
                Request::put(REPOSITORY_SELECTIONS_HTTP_PATH),
                &session,
                true,
            )
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({
                    "sources": [{
                        "sourceId": connection_id,
                        "repositoryIds": ["repository-1"]
                    }]
                })
                .to_string(),
            ))?,
        )
        .await?;
    assert_eq!(selection.status(), StatusCode::OK);

    let synchronization = app
        .clone()
        .oneshot(
            authenticated(Request::post(SYNCHRONIZATION_HTTP_PATH), &session, true)
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(synchronization.status(), StatusCode::OK);
    let body = synchronization.into_body().collect().await?.to_bytes();
    let decoded: SynchronizationResponse = serde_json::from_slice(&body)?;
    assert_eq!(decoded.selected_repository_count, 1);
    assert_eq!(decoded.synchronized_repository_count, 1);
    assert_eq!(decoded.failed_repository_count, 0);
    assert!(!decoded.already_running);

    let settings = app
        .clone()
        .oneshot(
            authenticated(Request::get(SETTINGS_HTTP_PATH), &session, false).body(Body::empty())?,
        )
        .await?;
    assert_eq!(settings.status(), StatusCode::OK);
    let body = settings.into_body().collect().await?.to_bytes();
    let decoded: MonitoringSettingsResponse = serde_json::from_slice(&body)?;
    assert_eq!(decoded.synchronization_interval_seconds, 60);
    assert_eq!(decoded.recent_runs_per_workflow, 10);

    let settings = app
        .clone()
        .oneshot(
            authenticated(Request::put(SETTINGS_HTTP_PATH), &session, true)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"synchronizationIntervalSeconds":120,"recentRunsPerWorkflow":5}"#,
                ))?,
        )
        .await?;
    assert_eq!(settings.status(), StatusCode::OK);
    let body = settings.into_body().collect().await?.to_bytes();
    let decoded: MonitoringSettingsResponse = serde_json::from_slice(&body)?;
    assert_eq!(decoded.synchronization_interval_seconds, 120);
    assert_eq!(decoded.recent_runs_per_workflow, 5);

    let synchronization = app
        .clone()
        .oneshot(
            authenticated(Request::get(SYNCHRONIZATION_HTTP_PATH), &session, false)
                .body(Body::empty())?,
        )
        .await?;
    let body = synchronization.into_body().collect().await?.to_bytes();
    let decoded: SynchronizationStatusResponse = serde_json::from_slice(&body)?;
    assert!(!decoded.running);
    assert!(decoded.last_completed_at.is_some());

    let repositories = app
        .clone()
        .oneshot(
            authenticated(Request::get(REPOSITORIES_HTTP_PATH), &session, false)
                .body(Body::empty())?,
        )
        .await?;
    let body = repositories.into_body().collect().await?.to_bytes();
    let decoded: ListRepositoriesResponse = serde_json::from_slice(&body)?;
    assert!(decoded.sources[0].repositories[0].selected);

    let workflows = app
        .clone()
        .oneshot(
            authenticated(Request::get(WORKFLOWS_HTTP_PATH), &session, false).body(Body::empty())?,
        )
        .await?;
    assert_eq!(workflows.status(), StatusCode::OK);
    let body = workflows.into_body().collect().await?.to_bytes();
    let decoded: ListWorkflowsResponse = serde_json::from_slice(&body)?;
    assert_eq!(decoded.selected_repository_count, 1);
    assert_eq!(decoded.workflows.len(), 1);
    assert_eq!(decoded.workflows[0].name, "Build");
    assert_eq!(decoded.workflows[0].repository_name, "example-project");
    assert_eq!(decoded.workflows[0].runs.len(), 5);
    assert_eq!(
        decoded.workflows[0].runs[0].outcome,
        opsscope_core::contracts::RunOutcome::Success
    );

    let logs = app
        .clone()
        .oneshot(
            authenticated(Request::post(WORKFLOW_RUN_LOGS_HTTP_PATH), &session, true)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "sourceId": connection_id,
                        "repositoryId": "repository-1",
                        "runId": "run-1",
                        "attempt": 1
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(logs.status(), StatusCode::OK);
    let body = logs.into_body().collect().await?.to_bytes();
    let decoded: WorkflowRunLogsResponse = serde_json::from_slice(&body)?;
    assert_eq!(decoded.files[0].name, "build.txt");
    assert_eq!(decoded.files[0].content, "Build completed");
    assert!(!decoded.truncated);

    let disconnected = app
        .clone()
        .oneshot(
            authenticated(Request::delete(CONNECTIONS_HTTP_PATH), &session, true)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"connectionId": connection_id}).to_string(),
                ))?,
        )
        .await?;
    assert_eq!(disconnected.status(), StatusCode::OK);

    let saved = app
        .oneshot(
            authenticated(Request::get(SOURCES_HTTP_PATH), &session, false).body(Body::empty())?,
        )
        .await?;
    let body = saved.into_body().collect().await?.to_bytes();
    let decoded: ListSourcesResponse = serde_json::from_slice(&body)?;
    assert!(decoded.sources[0].connections.is_empty());
    Ok(())
}

#[tokio::test]
async fn protected_routes_reject_anonymous_and_missing_csrf_requests()
-> Result<(), Box<dyn std::error::Error>> {
    let database = SqliteDatabase::in_memory()?;
    let secrets = EncryptedSecretStore::new(database.clone(), ServerMasterKey::generate()?);
    let authentication = WebAuthentication::for_tests();
    let session = authentication.issue_test_session();
    let app = router(test_dependencies(&database, secrets, authentication));

    for (method, path) in [
        (Method::GET, WORKFLOWS_HTTP_PATH),
        (Method::POST, WORKFLOW_RUN_LOGS_HTTP_PATH),
        (Method::GET, SETTINGS_HTTP_PATH),
        (Method::PUT, SETTINGS_HTTP_PATH),
        (Method::GET, SYNCHRONIZATION_HTTP_PATH),
        (Method::POST, SYNCHRONIZATION_HTTP_PATH),
        (Method::POST, CONNECTIONS_HTTP_PATH),
        (Method::DELETE, CONNECTIONS_HTTP_PATH),
        (Method::GET, SOURCES_HTTP_PATH),
        (Method::GET, REPOSITORIES_HTTP_PATH),
        (Method::PUT, REPOSITORY_SELECTIONS_HTTP_PATH),
        (Method::POST, crate::auth::AUTH_LOGOUT_PATH),
    ] {
        let anonymous = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED, "{path}");
    }

    let missing_csrf = app
        .clone()
        .oneshot(
            authenticated(Request::post(SYNCHRONIZATION_HTTP_PATH), &session, false)
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(missing_csrf.status(), StatusCode::FORBIDDEN);

    let invalid_csrf = app
        .clone()
        .oneshot(
            authenticated(Request::post(SYNCHRONIZATION_HTTP_PATH), &session, false)
                .header(CSRF_HEADER, "invalid")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(invalid_csrf.status(), StatusCode::FORBIDDEN);

    let auth_status = app
        .oneshot(
            authenticated(Request::get(AUTH_SESSION_PATH), &session, false).body(Body::empty())?,
        )
        .await?;
    assert_eq!(auth_status.status(), StatusCode::OK);
    let body = auth_status.into_body().collect().await?.to_bytes();
    let decoded: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(decoded["authenticated"], true);
    assert_eq!(decoded["user"]["subject"], "test-user");
    assert_eq!(decoded["csrfToken"], session.csrf_token);
    Ok(())
}

#[tokio::test]
async fn routes_are_available_without_sessions_when_oidc_is_disabled()
-> Result<(), Box<dyn std::error::Error>> {
    let database = SqliteDatabase::in_memory()?;
    let secrets = EncryptedSecretStore::new(database.clone(), ServerMasterKey::generate()?);
    let app = router(test_dependencies(
        &database,
        secrets,
        WebAuthentication::disabled_for_tests(),
    ));

    let session = app
        .clone()
        .oneshot(Request::get(AUTH_SESSION_PATH).body(Body::empty())?)
        .await?;
    assert_eq!(session.status(), StatusCode::OK);
    let body = session.into_body().collect().await?.to_bytes();
    let status: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(status["enabled"], false);
    assert_eq!(status["authenticated"], false);

    let settings = app
        .oneshot(
            Request::put(SETTINGS_HTTP_PATH)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"synchronizationIntervalSeconds":120,"recentRunsPerWorkflow":5}"#,
                ))?,
        )
        .await?;
    assert_eq!(settings.status(), StatusCode::OK);
    Ok(())
}
