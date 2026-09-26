#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use ciwatcher_core::application::{
    ConnectSource, DisconnectSource, ListRepositories, ListSources, ListWorkflows,
    PersistenceFailure, ProviderToken, SaveRepositorySelection, SecretReference, SecretStore,
};
use ciwatcher_core::contracts::{
    ConnectSourceRequest, ConnectionSummary, ConnectionValidationErrorResponse,
    DisconnectSourceRequest, DisconnectSourceResponse, HealthResponse, ListRepositoriesResponse,
    ListSourcesResponse, ListWorkflowsResponse, RepositorySelectionErrorResponse,
    SaveRepositorySelectionRequest, SaveRepositorySelectionResponse,
};
use ciwatcher_core::integrations::registered_sources;
use ciwatcher_core::persistence::SqliteDatabase;
use std::fs;
use std::sync::Arc;
use tauri::{Manager, State};

struct DesktopState {
    list_workflows: ListWorkflows,
    connect_source: ConnectSource,
    list_sources: ListSources,
    list_repositories: ListRepositories,
    save_repository_selection: SaveRepositorySelection,
    disconnect_source: DisconnectSource,
}

#[derive(Clone, Copy)]
struct KeyringSecretStore;

impl KeyringSecretStore {
    fn entry(reference: &SecretReference) -> Result<keyring::Entry, PersistenceFailure> {
        keyring::Entry::new("dev.ciwatcher.desktop", reference.expose())
            .map_err(|_| PersistenceFailure)
    }
}

impl SecretStore for KeyringSecretStore {
    fn store(
        &self,
        reference: &SecretReference,
        token: &ProviderToken,
    ) -> Result<(), PersistenceFailure> {
        Self::entry(reference)?
            .set_password(token.expose())
            .map_err(|_| PersistenceFailure)
    }

    fn retrieve(&self, reference: &SecretReference) -> Result<ProviderToken, PersistenceFailure> {
        Self::entry(reference)?
            .get_password()
            .map(ProviderToken::new)
            .map_err(|_| PersistenceFailure)
    }

    fn delete(&self, reference: &SecretReference) -> Result<(), PersistenceFailure> {
        match Self::entry(reference)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(PersistenceFailure),
        }
    }
}

#[tauri::command]
fn health() -> HealthResponse {
    HealthResponse::ready()
}

#[tauri::command]
async fn list_workflows(
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
async fn connect_source(
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
fn list_sources(state: State<'_, DesktopState>) -> Result<ListSourcesResponse, String> {
    state
        .list_sources
        .execute()
        .map(ListSourcesResponse::from_domain)
        .map_err(|_| "connection storage unavailable".to_owned())
}

#[tauri::command]
async fn list_repositories(
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
fn save_repository_selection(
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
fn disconnect_source(
    state: State<'_, DesktopState>,
    request: DisconnectSourceRequest,
) -> Result<DisconnectSourceResponse, String> {
    state
        .disconnect_source
        .execute(&request.source_id)
        .map(|disconnected| DisconnectSourceResponse { disconnected })
        .map_err(|_| "connection storage unavailable".to_owned())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&data_dir)?;
            let database = Arc::new(SqliteDatabase::open(data_dir.join("ciwatcher.sqlite3"))?);
            let connections = database.clone();
            let sources = registered_sources()?;
            let secrets = Arc::new(KeyringSecretStore);
            app.manage(DesktopState {
                list_workflows: ListWorkflows::new(
                    sources.clone(),
                    connections.clone(),
                    secrets.clone(),
                    database.clone(),
                ),
                connect_source: ConnectSource::new(
                    sources.clone(),
                    connections.clone(),
                    secrets.clone(),
                ),
                list_sources: ListSources::new(sources.clone(), connections.clone()),
                list_repositories: ListRepositories::new(
                    sources.clone(),
                    connections.clone(),
                    secrets.clone(),
                    database.clone(),
                ),
                save_repository_selection: SaveRepositorySelection::new(
                    sources.clone(),
                    connections.clone(),
                    database.clone(),
                ),
                disconnect_source: DisconnectSource::new(sources, connections, secrets),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            health,
            list_workflows,
            connect_source,
            list_sources,
            list_repositories,
            save_repository_selection,
            disconnect_source
        ])
        .run(tauri::generate_context!())
        .expect("desktop runtime failed");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_uses_the_shared_contract() {
        assert_eq!(health(), HealthResponse::ready());
    }
}
