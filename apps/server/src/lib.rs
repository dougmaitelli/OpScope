//! Development-only HTTP composition. Authentication is added before exposure.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use ciwatcher_core::application::{
    ConnectSource, ConnectSourceFailure, ConnectionRepository, DisconnectSource, ListRepositories,
    ListRepositoriesFailure, ListSources, ListWorkflows, ListWorkflowsFailure,
    RepositorySelectionRepository, SaveRepositorySelection, SaveRepositorySelectionFailure,
    SecretStore, SourceRegistry,
};
use ciwatcher_core::contracts::{
    CONNECTIONS_HTTP_PATH, ConnectSourceRequest, ConnectionSummary,
    ConnectionValidationErrorResponse, DisconnectSourceRequest, DisconnectSourceResponse,
    HEALTH_HTTP_PATH, HealthResponse, ListRepositoriesResponse, ListSourcesResponse,
    ListWorkflowsResponse, REPOSITORIES_HTTP_PATH, REPOSITORY_SELECTIONS_HTTP_PATH,
    RepositorySelectionErrorResponse, SOURCES_HTTP_PATH, SaveRepositorySelectionRequest,
    SaveRepositorySelectionResponse, WORKFLOWS_HTTP_PATH,
};
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    list_workflows: ListWorkflows,
    connect_source: ConnectSource,
    list_sources: ListSources,
    list_repositories: ListRepositories,
    save_repository_selection: SaveRepositorySelection,
    disconnect_source: DisconnectSource,
}

pub fn router(
    registry: SourceRegistry,
    connections: Arc<dyn ConnectionRepository>,
    secrets: Arc<dyn SecretStore>,
    repository_selections: Arc<dyn RepositorySelectionRepository>,
) -> Router {
    let state = AppState {
        list_workflows: ListWorkflows::new(
            registry.clone(),
            connections.clone(),
            secrets.clone(),
            repository_selections.clone(),
        ),
        connect_source: ConnectSource::new(registry.clone(), connections.clone(), secrets.clone()),
        list_sources: ListSources::new(registry.clone(), connections.clone()),
        list_repositories: ListRepositories::new(
            registry.clone(),
            connections.clone(),
            secrets.clone(),
            repository_selections.clone(),
        ),
        save_repository_selection: SaveRepositorySelection::new(
            registry.clone(),
            connections.clone(),
            repository_selections,
        ),
        disconnect_source: DisconnectSource::new(registry, connections, secrets),
    };

    Router::new()
        .route(HEALTH_HTTP_PATH, get(health))
        .route(WORKFLOWS_HTTP_PATH, get(list_workflows))
        .route(
            CONNECTIONS_HTTP_PATH,
            post(connect_source).delete(disconnect_source),
        )
        .route(SOURCES_HTTP_PATH, get(list_sources))
        .route(REPOSITORIES_HTTP_PATH, get(list_repositories))
        .route(
            REPOSITORY_SELECTIONS_HTTP_PATH,
            put(save_repository_selection),
        )
        .with_state(state)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse::ready())
}

async fn list_workflows(
    State(state): State<AppState>,
) -> Result<Json<ListWorkflowsResponse>, (StatusCode, Json<ConnectionValidationErrorResponse>)> {
    state
        .list_workflows
        .execute()
        .await
        .map(ListWorkflowsResponse::from_domain)
        .map(Json)
        .map_err(http_workflow_error)
}

async fn connect_source(
    State(state): State<AppState>,
    Json(mut request): Json<ConnectSourceRequest>,
) -> Result<Json<ConnectionSummary>, (StatusCode, Json<ConnectionValidationErrorResponse>)> {
    let credential = std::mem::take(&mut request.credential);
    state
        .connect_source
        .execute(&request.source_id, credential)
        .await
        .map(ConnectionSummary::from)
        .map(Json)
        .map_err(http_validation_error)
}

fn http_validation_error(
    failure: ConnectSourceFailure,
) -> (StatusCode, Json<ConnectionValidationErrorResponse>) {
    let status = match failure {
        ConnectSourceFailure::UnknownSource => StatusCode::BAD_REQUEST,
        ConnectSourceFailure::Validation(
            ciwatcher_core::application::ConnectionValidationFailure::InvalidCredentials,
        ) => StatusCode::UNAUTHORIZED,
        ConnectSourceFailure::Validation(
            ciwatcher_core::application::ConnectionValidationFailure::PermissionDenied,
        ) => StatusCode::FORBIDDEN,
        ConnectSourceFailure::Validation(
            ciwatcher_core::application::ConnectionValidationFailure::RateLimited,
        ) => StatusCode::TOO_MANY_REQUESTS,
        ConnectSourceFailure::Validation(
            ciwatcher_core::application::ConnectionValidationFailure::ProviderUnavailable
            | ciwatcher_core::application::ConnectionValidationFailure::UnexpectedResponse,
        ) => StatusCode::BAD_GATEWAY,
        ConnectSourceFailure::StorageUnavailable => StatusCode::SERVICE_UNAVAILABLE,
    };
    (status, Json(failure.into()))
}

async fn list_sources(
    State(state): State<AppState>,
) -> Result<Json<ListSourcesResponse>, StatusCode> {
    state
        .list_sources
        .execute()
        .map(ListSourcesResponse::from_domain)
        .map(Json)
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}

async fn list_repositories(
    State(state): State<AppState>,
) -> Result<Json<ListRepositoriesResponse>, (StatusCode, Json<ConnectionValidationErrorResponse>)> {
    state
        .list_repositories
        .execute()
        .await
        .map(ListRepositoriesResponse::from_domain)
        .map(Json)
        .map_err(http_repository_error)
}

fn http_repository_error(
    failure: ListRepositoriesFailure,
) -> (StatusCode, Json<ConnectionValidationErrorResponse>) {
    let status = match failure {
        ListRepositoriesFailure::Source(
            ciwatcher_core::application::ConnectionValidationFailure::InvalidCredentials,
        ) => StatusCode::UNAUTHORIZED,
        ListRepositoriesFailure::Source(
            ciwatcher_core::application::ConnectionValidationFailure::PermissionDenied,
        ) => StatusCode::FORBIDDEN,
        ListRepositoriesFailure::Source(
            ciwatcher_core::application::ConnectionValidationFailure::RateLimited,
        ) => StatusCode::TOO_MANY_REQUESTS,
        ListRepositoriesFailure::Source(
            ciwatcher_core::application::ConnectionValidationFailure::ProviderUnavailable
            | ciwatcher_core::application::ConnectionValidationFailure::UnexpectedResponse,
        ) => StatusCode::BAD_GATEWAY,
        ListRepositoriesFailure::StorageUnavailable => StatusCode::SERVICE_UNAVAILABLE,
    };
    (status, Json(failure.into()))
}

fn http_workflow_error(
    failure: ListWorkflowsFailure,
) -> (StatusCode, Json<ConnectionValidationErrorResponse>) {
    let status = match failure {
        ListWorkflowsFailure::Source(
            ciwatcher_core::application::ConnectionValidationFailure::InvalidCredentials,
        ) => StatusCode::UNAUTHORIZED,
        ListWorkflowsFailure::Source(
            ciwatcher_core::application::ConnectionValidationFailure::PermissionDenied,
        ) => StatusCode::FORBIDDEN,
        ListWorkflowsFailure::Source(
            ciwatcher_core::application::ConnectionValidationFailure::RateLimited,
        ) => StatusCode::TOO_MANY_REQUESTS,
        ListWorkflowsFailure::Source(
            ciwatcher_core::application::ConnectionValidationFailure::ProviderUnavailable
            | ciwatcher_core::application::ConnectionValidationFailure::UnexpectedResponse,
        ) => StatusCode::BAD_GATEWAY,
        ListWorkflowsFailure::StorageUnavailable => StatusCode::SERVICE_UNAVAILABLE,
    };
    (status, Json(failure.into()))
}

async fn save_repository_selection(
    State(state): State<AppState>,
    Json(request): Json<SaveRepositorySelectionRequest>,
) -> Result<
    Json<SaveRepositorySelectionResponse>,
    (StatusCode, Json<RepositorySelectionErrorResponse>),
> {
    state
        .save_repository_selection
        .execute(&request.into_domain())
        .map(|selected_count| Json(SaveRepositorySelectionResponse { selected_count }))
        .map_err(|failure| {
            let status = match failure {
                SaveRepositorySelectionFailure::InvalidSelection => StatusCode::BAD_REQUEST,
                SaveRepositorySelectionFailure::SourceNotConnected => StatusCode::CONFLICT,
                SaveRepositorySelectionFailure::StorageUnavailable => {
                    StatusCode::SERVICE_UNAVAILABLE
                }
            };
            (status, Json(failure.into()))
        })
}

async fn disconnect_source(
    State(state): State<AppState>,
    Json(request): Json<DisconnectSourceRequest>,
) -> Result<Json<DisconnectSourceResponse>, StatusCode> {
    state
        .disconnect_source
        .execute(&request.source_id)
        .map(|disconnected| Json(DisconnectSourceResponse { disconnected }))
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use axum::body::Body;
    use axum::http::{Request, header};
    use ciwatcher_core::application::{
        ConnectionValidationFailure, CredentialField, ProviderToken, SourceDescriptor,
        SourceModule, ValidatedAccount,
    };
    use ciwatcher_core::domain::{Repository, RepositoryVisibility, Workflow, WorkflowState};
    use ciwatcher_core::persistence::{EncryptedSecretStore, ServerMasterKey, SqliteDatabase};
    use http_body_util::BodyExt;
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
                web_url:
                    "https://example.com/example-user/example-project/actions/workflows/build.yml"
                        .to_owned(),
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
}
