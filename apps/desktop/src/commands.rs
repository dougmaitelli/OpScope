use crate::state::DesktopState;
use opsscope_core::contracts::{
    ChangeRequestDetailsErrorResponse, ChangeRequestDetailsRequest, ChangeRequestDetailsResponse,
    ConnectSourceRequest, ConnectionSummary, ConnectionValidationErrorResponse,
    DisconnectSourceRequest, DisconnectSourceResponse, HealthResponse, IssueDetailsErrorResponse,
    IssueDetailsRequest, IssueDetailsResponse, ListActivityResponse, ListChangeRequestsResponse,
    ListIssuesResponse, ListRepositoriesResponse, ListSourcesResponse, ListWorkflowsResponse,
    MonitoringSettingsErrorResponse, MonitoringSettingsResponse, RepositorySelectionErrorResponse,
    SaveRepositorySelectionRequest, SaveRepositorySelectionResponse, SynchronizationResponse,
    SynchronizationStatusResponse, UpdateMonitoringSettingsRequest, UpdateStatusResponse,
    WorkflowRunLogsErrorResponse, WorkflowRunLogsRequest, WorkflowRunLogsResponse,
};
use tauri::State;

#[tauri::command]
pub(crate) fn health() -> HealthResponse {
    HealthResponse::ready()
}

#[tauri::command]
pub(crate) async fn update_status(
    state: State<'_, DesktopState>,
) -> Result<UpdateStatusResponse, String> {
    state
        .check_for_updates
        .execute()
        .await
        .map(UpdateStatusResponse::from)
        .map_err(|failure| failure.to_string())
}

#[tauri::command]
pub(crate) async fn list_workflows(
    state: State<'_, DesktopState>,
) -> Result<ListWorkflowsResponse, ConnectionValidationErrorResponse> {
    state
        .list_workflows
        .execute()
        .await
        .map(ListWorkflowsResponse::from_domain)
        .map_err(ConnectionValidationErrorResponse::from)
}

#[tauri::command]
pub(crate) async fn list_change_requests(
    state: State<'_, DesktopState>,
) -> Result<ListChangeRequestsResponse, ConnectionValidationErrorResponse> {
    state
        .list_change_requests
        .execute()
        .await
        .map(ListChangeRequestsResponse::from_domain)
        .map_err(ConnectionValidationErrorResponse::from)
}

#[tauri::command]
pub(crate) async fn list_issues(
    state: State<'_, DesktopState>,
) -> Result<ListIssuesResponse, ConnectionValidationErrorResponse> {
    state
        .list_issues
        .execute()
        .await
        .map(ListIssuesResponse::from_domain)
        .map_err(ConnectionValidationErrorResponse::from)
}

#[tauri::command]
pub(crate) fn list_activity(
    state: State<'_, DesktopState>,
) -> Result<ListActivityResponse, ConnectionValidationErrorResponse> {
    state
        .list_activity
        .execute()
        .map(ListActivityResponse::from_domain)
        .map_err(|_| {
            ConnectionValidationErrorResponse::from(
                opsscope_core::application::ListWorkflowsFailure::StorageUnavailable,
            )
        })
}

#[tauri::command]
pub(crate) async fn change_request_details(
    state: State<'_, DesktopState>,
    request: ChangeRequestDetailsRequest,
) -> Result<ChangeRequestDetailsResponse, ChangeRequestDetailsErrorResponse> {
    state
        .change_request_details
        .execute(&request.source_id, &request.repository_id, request.number)
        .await
        .map(ChangeRequestDetailsResponse::from)
        .map_err(ChangeRequestDetailsErrorResponse::from)
}

#[tauri::command]
pub(crate) async fn issue_details(
    state: State<'_, DesktopState>,
    request: IssueDetailsRequest,
) -> Result<IssueDetailsResponse, IssueDetailsErrorResponse> {
    state
        .issue_details
        .execute(&request.source_id, &request.repository_id, request.number)
        .await
        .map(IssueDetailsResponse::from)
        .map_err(IssueDetailsErrorResponse::from)
}

#[tauri::command]
pub(crate) async fn workflow_run_logs(
    state: State<'_, DesktopState>,
    request: WorkflowRunLogsRequest,
) -> Result<WorkflowRunLogsResponse, WorkflowRunLogsErrorResponse> {
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
        .map_err(WorkflowRunLogsErrorResponse::from)
}

#[tauri::command]
pub(crate) async fn synchronize_sources(
    state: State<'_, DesktopState>,
) -> Result<SynchronizationResponse, ConnectionValidationErrorResponse> {
    state
        .synchronize_sources
        .execute()
        .await
        .map(SynchronizationResponse::from)
        .map_err(ConnectionValidationErrorResponse::from)
}

#[tauri::command]
pub(crate) fn synchronization_status(
    state: State<'_, DesktopState>,
) -> SynchronizationStatusResponse {
    state.synchronize_sources.status().into()
}

#[tauri::command]
pub(crate) fn get_settings(
    state: State<'_, DesktopState>,
) -> Result<MonitoringSettingsResponse, MonitoringSettingsErrorResponse> {
    state
        .get_settings
        .execute()
        .map(MonitoringSettingsResponse::from)
        .map_err(MonitoringSettingsErrorResponse::from)
}

#[tauri::command]
pub(crate) fn update_settings(
    state: State<'_, DesktopState>,
    request: UpdateMonitoringSettingsRequest,
) -> Result<MonitoringSettingsResponse, MonitoringSettingsErrorResponse> {
    state
        .update_settings
        .execute(request.into())
        .map(MonitoringSettingsResponse::from)
        .map_err(MonitoringSettingsErrorResponse::from)
}

#[tauri::command]
pub(crate) async fn connect_source(
    state: State<'_, DesktopState>,
    mut request: ConnectSourceRequest,
) -> Result<ConnectionSummary, ConnectionValidationErrorResponse> {
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
        .map_err(ConnectionValidationErrorResponse::from)
}

#[tauri::command]
pub(crate) fn list_sources(state: State<'_, DesktopState>) -> Result<ListSourcesResponse, String> {
    state
        .list_sources
        .execute()
        .map(ListSourcesResponse::from_domain)
        .map_err(|_| "connection storage unavailable".to_owned())
}

#[tauri::command]
pub(crate) async fn list_repositories(
    state: State<'_, DesktopState>,
) -> Result<ListRepositoriesResponse, ConnectionValidationErrorResponse> {
    state
        .list_repositories
        .execute()
        .await
        .map(ListRepositoriesResponse::from_domain)
        .map_err(ConnectionValidationErrorResponse::from)
}

#[tauri::command]
pub(crate) fn save_repository_selection(
    state: State<'_, DesktopState>,
    request: SaveRepositorySelectionRequest,
) -> Result<SaveRepositorySelectionResponse, RepositorySelectionErrorResponse> {
    state
        .save_repository_selection
        .execute(&request.into_domain())
        .map(|selected_count| SaveRepositorySelectionResponse {
            selected_count,
        })
        .map_err(RepositorySelectionErrorResponse::from)
}

#[tauri::command]
pub(crate) fn disconnect_source(
    state: State<'_, DesktopState>,
    request: DisconnectSourceRequest,
) -> Result<DisconnectSourceResponse, String> {
    state
        .disconnect_source
        .execute(&request.connection_id)
        .map(|disconnected| DisconnectSourceResponse {
            disconnected,
        })
        .map_err(|_| "connection storage unavailable".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_uses_the_shared_contract() {
        assert_eq!(health(), HealthResponse::ready());
    }
}
