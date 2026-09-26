use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use ciwatcher_core::application::{
    ConnectSource, ConnectSourceFailure, ConnectionRepository, DisconnectSource, ListRepositories,
    ListRepositoriesFailure, ListSources, ListWorkflows, ListWorkflowsFailure,
    RepositorySelectionRepository, SaveRepositorySelection, SaveRepositorySelectionFailure,
    SecretStore, SourceRegistry,
};
use ciwatcher_core::contracts::{
    ConnectSourceRequest, ConnectionSummary, ConnectionValidationErrorResponse,
    DisconnectSourceRequest, DisconnectSourceResponse, HealthResponse, ListRepositoriesResponse,
    ListSourcesResponse, ListWorkflowsResponse, RepositorySelectionErrorResponse,
    SaveRepositorySelectionRequest, SaveRepositorySelectionResponse,
};
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct AppState {
    list_workflows: ListWorkflows,
    connect_source: ConnectSource,
    list_sources: ListSources,
    list_repositories: ListRepositories,
    save_repository_selection: SaveRepositorySelection,
    disconnect_source: DisconnectSource,
}

impl AppState {
    pub(crate) fn new(
        registry: SourceRegistry,
        connections: Arc<dyn ConnectionRepository>,
        secrets: Arc<dyn SecretStore>,
        repository_selections: Arc<dyn RepositorySelectionRepository>,
    ) -> Self {
        Self {
            list_workflows: ListWorkflows::new(
                registry.clone(),
                connections.clone(),
                secrets.clone(),
                repository_selections.clone(),
            ),
            connect_source: ConnectSource::new(
                registry.clone(),
                connections.clone(),
                secrets.clone(),
            ),
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
        }
    }
}

pub(crate) async fn health() -> Json<HealthResponse> {
    Json(HealthResponse::ready())
}

pub(crate) async fn list_workflows(
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

pub(crate) async fn connect_source(
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

pub(crate) async fn list_sources(
    State(state): State<AppState>,
) -> Result<Json<ListSourcesResponse>, StatusCode> {
    state
        .list_sources
        .execute()
        .map(ListSourcesResponse::from_domain)
        .map(Json)
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}

pub(crate) async fn list_repositories(
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

pub(crate) async fn save_repository_selection(
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

pub(crate) async fn disconnect_source(
    State(state): State<AppState>,
    Json(request): Json<DisconnectSourceRequest>,
) -> Result<Json<DisconnectSourceResponse>, StatusCode> {
    state
        .disconnect_source
        .execute(&request.source_id)
        .map(|disconnected| Json(DisconnectSourceResponse { disconnected }))
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}
