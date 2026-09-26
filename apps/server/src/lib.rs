//! Development-only HTTP composition. Authentication is added before exposure.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use ciwatcher_core::application::{
    ConnectSource, ConnectSourceFailure, ConnectionRepository, DisconnectSource, ListMonitors,
    ListRepositories, ListRepositoriesFailure, ListSources, MonitorSource,
    RepositorySelectionRepository, SaveRepositorySelection, SaveRepositorySelectionFailure,
    SecretStore, SourceRegistry,
};
use ciwatcher_core::contracts::{
    CONNECTIONS_HTTP_PATH, ConnectSourceRequest, ConnectionSummary,
    ConnectionValidationErrorResponse, DisconnectSourceRequest, DisconnectSourceResponse,
    HEALTH_HTTP_PATH, HealthResponse, LIST_MONITORS_HTTP_PATH, ListMonitorsResponse,
    ListRepositoriesResponse, ListSourcesResponse, REPOSITORIES_HTTP_PATH,
    REPOSITORY_SELECTIONS_HTTP_PATH, RepositorySelectionErrorResponse, SOURCES_HTTP_PATH,
    SaveRepositorySelectionRequest, SaveRepositorySelectionResponse,
};
use std::sync::Arc;

#[derive(Clone)]
struct AppState<S> {
    list_monitors: ListMonitors<S>,
    connect_source: ConnectSource,
    list_sources: ListSources,
    list_repositories: ListRepositories,
    save_repository_selection: SaveRepositorySelection,
    disconnect_source: DisconnectSource,
}

pub fn router<S>(
    source: S,
    registry: SourceRegistry,
    connections: Arc<dyn ConnectionRepository>,
    secrets: Arc<dyn SecretStore>,
    repository_selections: Arc<dyn RepositorySelectionRepository>,
) -> Router
where
    S: MonitorSource + Clone + 'static,
{
    let state = AppState {
        list_monitors: ListMonitors::new(source),
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
        .route(LIST_MONITORS_HTTP_PATH, get(list_monitors::<S>))
        .route(
            CONNECTIONS_HTTP_PATH,
            post(connect_source::<S>).delete(disconnect_source::<S>),
        )
        .route(SOURCES_HTTP_PATH, get(list_sources::<S>))
        .route(REPOSITORIES_HTTP_PATH, get(list_repositories::<S>))
        .route(
            REPOSITORY_SELECTIONS_HTTP_PATH,
            put(save_repository_selection::<S>),
        )
        .with_state(state)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse::ready())
}

async fn list_monitors<S>(
    State(state): State<AppState<S>>,
) -> Result<Json<ListMonitorsResponse>, StatusCode>
where
    S: MonitorSource,
{
    state
        .list_monitors
        .execute()
        .await
        .map(ListMonitorsResponse::from_domain)
        .map(Json)
        .map_err(|_| StatusCode::BAD_GATEWAY)
}

async fn connect_source<S>(
    State(state): State<AppState<S>>,
    Json(mut request): Json<ConnectSourceRequest>,
) -> Result<Json<ConnectionSummary>, (StatusCode, Json<ConnectionValidationErrorResponse>)>
where
    S: MonitorSource,
{
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

async fn list_sources<S>(
    State(state): State<AppState<S>>,
) -> Result<Json<ListSourcesResponse>, StatusCode>
where
    S: MonitorSource,
{
    state
        .list_sources
        .execute()
        .map(ListSourcesResponse::from_domain)
        .map(Json)
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}

async fn list_repositories<S>(
    State(state): State<AppState<S>>,
) -> Result<Json<ListRepositoriesResponse>, (StatusCode, Json<ConnectionValidationErrorResponse>)>
where
    S: MonitorSource,
{
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

async fn save_repository_selection<S>(
    State(state): State<AppState<S>>,
    Json(request): Json<SaveRepositorySelectionRequest>,
) -> Result<
    Json<SaveRepositorySelectionResponse>,
    (StatusCode, Json<RepositorySelectionErrorResponse>),
>
where
    S: MonitorSource,
{
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

async fn disconnect_source<S>(
    State(state): State<AppState<S>>,
    Json(request): Json<DisconnectSourceRequest>,
) -> Result<Json<DisconnectSourceResponse>, StatusCode>
where
    S: MonitorSource,
{
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
    use ciwatcher_core::domain::{Repository, RepositoryVisibility};
    use ciwatcher_core::integrations::fake::FakeMonitorSource;
    use ciwatcher_core::persistence::{EncryptedSecretStore, ServerMasterKey, SqliteDatabase};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[derive(Clone)]
    struct FakeSourceModule;

    #[async_trait]
    impl SourceModule for FakeSourceModule {
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
    }

    fn test_registry() -> SourceRegistry {
        SourceRegistry::new(vec![Arc::new(FakeSourceModule)])
    }

    #[tokio::test]
    async fn both_routes_use_the_shared_contract() -> Result<(), Box<dyn std::error::Error>> {
        let database = SqliteDatabase::in_memory()?;
        let secrets = EncryptedSecretStore::new(database.clone(), ServerMasterKey::generate()?);
        let app = router(
            FakeMonitorSource,
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

        let monitors = app
            .oneshot(Request::get(LIST_MONITORS_HTTP_PATH).body(Body::empty())?)
            .await?;
        assert_eq!(monitors.status(), StatusCode::OK);
        let body = monitors.into_body().collect().await?.to_bytes();
        let decoded: ListMonitorsResponse = serde_json::from_slice(&body)?;
        assert_eq!(decoded.monitors.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn generic_source_routes_connect_list_and_disconnect_without_exposing_secrets()
    -> Result<(), Box<dyn std::error::Error>> {
        let database = SqliteDatabase::in_memory()?;
        let secrets = EncryptedSecretStore::new(database.clone(), ServerMasterKey::generate()?);
        let app = router(
            FakeMonitorSource,
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
