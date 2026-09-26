//! Development-only HTTP composition. Authentication is added before exposure.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use ciwatcher_core::application::{
    ConnectSource, ConnectSourceFailure, ConnectionRepository, DisconnectSource, ListMonitors,
    ListSources, MonitorSource, SecretStore, SourceRegistry,
};
use ciwatcher_core::contracts::{
    CONNECTIONS_HTTP_PATH, ConnectSourceRequest, ConnectionSummary,
    ConnectionValidationErrorResponse, DisconnectSourceRequest, DisconnectSourceResponse,
    HEALTH_HTTP_PATH, HealthResponse, LIST_MONITORS_HTTP_PATH, ListMonitorsResponse,
    ListSourcesResponse, SOURCES_HTTP_PATH,
};
use std::sync::Arc;

#[derive(Clone)]
struct AppState<S> {
    list_monitors: ListMonitors<S>,
    connect_source: ConnectSource,
    list_sources: ListSources,
    disconnect_source: DisconnectSource,
}

pub fn router<S>(
    source: S,
    registry: SourceRegistry,
    connections: Arc<dyn ConnectionRepository>,
    secrets: Arc<dyn SecretStore>,
) -> Router
where
    S: MonitorSource + Clone + 'static,
{
    let state = AppState {
        list_monitors: ListMonitors::new(source),
        connect_source: ConnectSource::new(registry.clone(), connections.clone(), secrets.clone()),
        list_sources: ListSources::new(registry.clone(), connections.clone()),
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
    }

    fn test_registry() -> SourceRegistry {
        SourceRegistry::new(vec![Arc::new(FakeSourceModule)])
    }

    #[tokio::test]
    async fn both_routes_use_the_shared_contract() -> Result<(), Box<dyn std::error::Error>> {
        let database = SqliteDatabase::in_memory()?;
        let app = router(
            FakeMonitorSource,
            test_registry(),
            Arc::new(database.clone()),
            Arc::new(EncryptedSecretStore::new(
                database,
                ServerMasterKey::generate()?,
            )),
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
        let app = router(
            FakeMonitorSource,
            test_registry(),
            Arc::new(database.clone()),
            Arc::new(EncryptedSecretStore::new(
                database,
                ServerMasterKey::generate()?,
            )),
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
