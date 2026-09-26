use crate::state::DesktopState;
use ciwatcher_core::contracts::{
    ConnectSourceRequest, ConnectionSummary, ConnectionValidationErrorResponse,
    DisconnectSourceRequest, DisconnectSourceResponse, HealthResponse, ListRepositoriesResponse,
    ListSourcesResponse, ListWorkflowsResponse, RepositorySelectionErrorResponse,
    SaveRepositorySelectionRequest, SaveRepositorySelectionResponse,
};
use tauri::State;

#[tauri::command]
pub(crate) fn health() -> HealthResponse {
    HealthResponse::ready()
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
pub(crate) async fn connect_source(
    state: State<'_, DesktopState>,
    mut request: ConnectSourceRequest,
) -> Result<ConnectionSummary, ConnectionValidationErrorResponse> {
    let credential = std::mem::take(&mut request.credential);
    state
        .connect_source
        .execute(&request.source_id, credential)
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
        .map(|selected_count| SaveRepositorySelectionResponse { selected_count })
        .map_err(RepositorySelectionErrorResponse::from)
}

#[tauri::command]
pub(crate) fn disconnect_source(
    state: State<'_, DesktopState>,
    request: DisconnectSourceRequest,
) -> Result<DisconnectSourceResponse, String> {
    state
        .disconnect_source
        .execute(&request.source_id)
        .map(|disconnected| DisconnectSourceResponse { disconnected })
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
