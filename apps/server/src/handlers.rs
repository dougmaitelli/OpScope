use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use opsscope_core::application::{
    CheckForUpdates, ConnectSource, ConnectSourceFailure, ConnectionRepository,
    ConnectionValidationFailure, DisconnectSource, GetMonitoringSettings, GetWorkflowRunLogs,
    ListChangeRequests, ListChangeRequestsFailure, ListRepositories, ListRepositoriesFailure,
    ListSources, ListWorkflows, ListWorkflowsFailure, NotifyRepositoryFailures,
    RepositorySelectionRepository, SaveRepositorySelection, SaveRepositorySelectionFailure,
    SecretStore, SettingsFailure, SettingsRepository, SourceRegistry, SynchronizeSources,
    UpdateMonitoringSettings, WorkflowRunLogsFailure,
};
use opsscope_core::contracts::{
    ConnectSourceRequest, ConnectionSummary, ConnectionValidationErrorResponse,
    DisconnectSourceRequest, DisconnectSourceResponse, HealthResponse, ListChangeRequestsResponse,
    ListRepositoriesResponse, ListSourcesResponse, ListWorkflowsResponse,
    MonitoringSettingsErrorResponse, MonitoringSettingsResponse, RepositorySelectionErrorResponse,
    SaveRepositorySelectionRequest, SaveRepositorySelectionResponse, SynchronizationResponse,
    SynchronizationStatusResponse, UpdateMonitoringSettingsRequest, UpdateStatusResponse,
    WorkflowRunLogsErrorResponse, WorkflowRunLogsRequest, WorkflowRunLogsResponse,
};
use opsscope_core::source_data::{ReadThroughSourceData, SourceDataCache, SourceDataCachePolicy};
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct AppState {
    check_for_updates: CheckForUpdates,
    list_workflows: ListWorkflows,
    list_change_requests: ListChangeRequests,
    workflow_run_logs: GetWorkflowRunLogs,
    connect_source: ConnectSource,
    list_sources: ListSources,
    list_repositories: ListRepositories,
    save_repository_selection: SaveRepositorySelection,
    disconnect_source: DisconnectSource,
    synchronize_sources: SynchronizeSources,
    get_settings: GetMonitoringSettings,
    update_settings: UpdateMonitoringSettings,
}

impl AppState {
    pub(crate) fn new(
        registry: SourceRegistry,
        connections: Arc<dyn ConnectionRepository>,
        secrets: Arc<dyn SecretStore>,
        repository_selections: Arc<dyn RepositorySelectionRepository>,
        source_data_cache: Arc<dyn SourceDataCache>,
        settings: Arc<dyn SettingsRepository>,
        failure_notifications: NotifyRepositoryFailures,
    ) -> Self {
        let source_data = Arc::new(ReadThroughSourceData::cached(
            registry.clone(),
            connections.clone(),
            secrets.clone(),
            source_data_cache,
            SourceDataCachePolicy::default(),
        ));
        Self {
            check_for_updates: CheckForUpdates::github(),
            list_workflows: ListWorkflows::new(
                source_data.clone(),
                repository_selections.clone(),
                settings.clone(),
            ),
            list_change_requests: ListChangeRequests::new(
                source_data.clone(),
                repository_selections.clone(),
            ),
            workflow_run_logs: GetWorkflowRunLogs::new(source_data.clone()),
            connect_source: ConnectSource::new(
                registry.clone(),
                connections.clone(),
                secrets.clone(),
            ),
            list_sources: ListSources::new(registry.clone(), connections.clone()),
            list_repositories: ListRepositories::new(
                source_data.clone(),
                repository_selections.clone(),
            ),
            synchronize_sources: SynchronizeSources::new(
                source_data,
                repository_selections.clone(),
                failure_notifications,
            ),
            save_repository_selection: SaveRepositorySelection::new(
                connections.clone(),
                repository_selections,
            ),
            disconnect_source: DisconnectSource::new(connections, secrets),
            get_settings: GetMonitoringSettings::new(settings.clone()),
            update_settings: UpdateMonitoringSettings::new(settings),
        }
    }

    pub(crate) fn synchronizer(&self) -> SynchronizeSources {
        self.synchronize_sources.clone()
    }

    pub(crate) fn settings_reader(&self) -> GetMonitoringSettings {
        self.get_settings.clone()
    }
}

pub(crate) async fn list_change_requests(
    State(state): State<AppState>,
) -> Result<Json<ListChangeRequestsResponse>, (StatusCode, Json<ConnectionValidationErrorResponse>)>
{
    state
        .list_change_requests
        .execute()
        .await
        .map(ListChangeRequestsResponse::from_domain)
        .map(Json)
        .map_err(http_change_requests_error)
}

fn http_change_requests_error(
    failure: ListChangeRequestsFailure,
) -> (StatusCode, Json<ConnectionValidationErrorResponse>) {
    eprintln!("failed to list change requests: {failure}");
    let status = match failure {
        ListChangeRequestsFailure::Source(ConnectionValidationFailure::InvalidCredentials) => {
            StatusCode::UNAUTHORIZED
        }
        ListChangeRequestsFailure::Source(ConnectionValidationFailure::PermissionDenied) => {
            StatusCode::FORBIDDEN
        }
        ListChangeRequestsFailure::Source(ConnectionValidationFailure::RateLimited) => {
            StatusCode::TOO_MANY_REQUESTS
        }
        ListChangeRequestsFailure::Source(_) => StatusCode::BAD_GATEWAY,
        ListChangeRequestsFailure::StorageUnavailable => StatusCode::SERVICE_UNAVAILABLE,
    };
    (status, Json(failure.into()))
}

pub(crate) async fn get_settings(
    State(state): State<AppState>,
) -> Result<Json<MonitoringSettingsResponse>, (StatusCode, Json<MonitoringSettingsErrorResponse>)> {
    state
        .get_settings
        .execute()
        .map(MonitoringSettingsResponse::from)
        .map(Json)
        .map_err(http_settings_error)
}

pub(crate) async fn update_settings(
    State(state): State<AppState>,
    Json(request): Json<UpdateMonitoringSettingsRequest>,
) -> Result<Json<MonitoringSettingsResponse>, (StatusCode, Json<MonitoringSettingsErrorResponse>)> {
    state
        .update_settings
        .execute(request.into())
        .map(MonitoringSettingsResponse::from)
        .map(Json)
        .map_err(http_settings_error)
}

fn http_settings_error(
    failure: SettingsFailure,
) -> (StatusCode, Json<MonitoringSettingsErrorResponse>) {
    let status = match failure {
        SettingsFailure::InvalidSettings => StatusCode::BAD_REQUEST,
        SettingsFailure::StorageUnavailable => StatusCode::SERVICE_UNAVAILABLE,
    };
    (status, Json(failure.into()))
}

pub(crate) async fn workflow_run_logs(
    State(state): State<AppState>,
    Json(request): Json<WorkflowRunLogsRequest>,
) -> Result<Json<WorkflowRunLogsResponse>, (StatusCode, Json<WorkflowRunLogsErrorResponse>)> {
    state
        .workflow_run_logs
        .execute(
            &request.source_id,
            &request.repository_id,
            &request.run_id,
            request.attempt,
        )
        .await
        .map(WorkflowRunLogsResponse::from)
        .map(Json)
        .map_err(http_run_logs_error)
}

fn http_run_logs_error(
    failure: WorkflowRunLogsFailure,
) -> (StatusCode, Json<WorkflowRunLogsErrorResponse>) {
    let status = match failure {
        WorkflowRunLogsFailure::UnknownSource
        | WorkflowRunLogsFailure::RepositoryNotFound
        | WorkflowRunLogsFailure::RunNotFound => StatusCode::NOT_FOUND,
        WorkflowRunLogsFailure::SourceNotConnected => StatusCode::CONFLICT,
        WorkflowRunLogsFailure::LogsUnavailable => StatusCode::NOT_FOUND,
        WorkflowRunLogsFailure::LogsTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        WorkflowRunLogsFailure::InvalidCredentials => StatusCode::UNAUTHORIZED,
        WorkflowRunLogsFailure::PermissionDenied => StatusCode::FORBIDDEN,
        WorkflowRunLogsFailure::RateLimited => StatusCode::TOO_MANY_REQUESTS,
        WorkflowRunLogsFailure::ProviderUnavailable
        | WorkflowRunLogsFailure::UnexpectedResponse => StatusCode::BAD_GATEWAY,
        WorkflowRunLogsFailure::StorageUnavailable => StatusCode::SERVICE_UNAVAILABLE,
    };
    (status, Json(failure.into()))
}

pub(crate) async fn synchronization_status(
    State(state): State<AppState>,
) -> Json<SynchronizationStatusResponse> {
    Json(state.synchronize_sources.status().into())
}

pub(crate) async fn synchronize_sources(
    State(state): State<AppState>,
) -> Result<Json<SynchronizationResponse>, (StatusCode, Json<ConnectionValidationErrorResponse>)> {
    state
        .synchronize_sources
        .execute()
        .await
        .map(SynchronizationResponse::from)
        .map(Json)
        .map_err(|failure| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ConnectionValidationErrorResponse::from(failure)),
            )
        })
}

pub(crate) async fn health() -> Json<HealthResponse> {
    Json(HealthResponse::ready())
}

pub(crate) async fn update_status(
    State(state): State<AppState>,
) -> Result<Json<UpdateStatusResponse>, StatusCode> {
    state
        .check_for_updates
        .execute()
        .await
        .map(UpdateStatusResponse::from)
        .map(Json)
        .map_err(|_| StatusCode::BAD_GATEWAY)
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
        .execute(
            &request.source_id,
            request.connection_id.as_deref(),
            &request.configuration,
            credential,
        )
        .await
        .map(ConnectionSummary::from)
        .map(Json)
        .map_err(http_validation_error)
}

fn http_validation_error(
    failure: ConnectSourceFailure,
) -> (StatusCode, Json<ConnectionValidationErrorResponse>) {
    let status = match failure {
        ConnectSourceFailure::UnknownSource
        | ConnectSourceFailure::UnknownConnection
        | ConnectSourceFailure::DuplicateConnection
        | ConnectSourceFailure::Validation(
            opsscope_core::application::ConnectionValidationFailure::InvalidConfiguration,
        ) => StatusCode::BAD_REQUEST,
        ConnectSourceFailure::Validation(
            opsscope_core::application::ConnectionValidationFailure::InvalidCredentials,
        ) => StatusCode::UNAUTHORIZED,
        ConnectSourceFailure::Validation(
            opsscope_core::application::ConnectionValidationFailure::PermissionDenied,
        ) => StatusCode::FORBIDDEN,
        ConnectSourceFailure::Validation(
            opsscope_core::application::ConnectionValidationFailure::RateLimited,
        ) => StatusCode::TOO_MANY_REQUESTS,
        ConnectSourceFailure::Validation(
            opsscope_core::application::ConnectionValidationFailure::ProviderUnavailable
            | opsscope_core::application::ConnectionValidationFailure::UnexpectedResponse,
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
            opsscope_core::application::ConnectionValidationFailure::InvalidCredentials,
        ) => StatusCode::UNAUTHORIZED,
        ListRepositoriesFailure::Source(
            opsscope_core::application::ConnectionValidationFailure::PermissionDenied,
        ) => StatusCode::FORBIDDEN,
        ListRepositoriesFailure::Source(
            opsscope_core::application::ConnectionValidationFailure::RateLimited,
        ) => StatusCode::TOO_MANY_REQUESTS,
        ListRepositoriesFailure::Source(
            opsscope_core::application::ConnectionValidationFailure::InvalidConfiguration
            | opsscope_core::application::ConnectionValidationFailure::ProviderUnavailable
            | opsscope_core::application::ConnectionValidationFailure::UnexpectedResponse,
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
            opsscope_core::application::ConnectionValidationFailure::InvalidCredentials,
        ) => StatusCode::UNAUTHORIZED,
        ListWorkflowsFailure::Source(
            opsscope_core::application::ConnectionValidationFailure::PermissionDenied,
        ) => StatusCode::FORBIDDEN,
        ListWorkflowsFailure::Source(
            opsscope_core::application::ConnectionValidationFailure::RateLimited,
        ) => StatusCode::TOO_MANY_REQUESTS,
        ListWorkflowsFailure::Source(
            opsscope_core::application::ConnectionValidationFailure::InvalidConfiguration
            | opsscope_core::application::ConnectionValidationFailure::ProviderUnavailable
            | opsscope_core::application::ConnectionValidationFailure::UnexpectedResponse,
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
        .execute(&request.connection_id)
        .map(|disconnected| Json(DisconnectSourceResponse { disconnected }))
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}
