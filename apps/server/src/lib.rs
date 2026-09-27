//! Development-only HTTP composition. Authentication is added before exposure.

mod handlers;
pub mod notifications;

use axum::Router;
use axum::routing::{get, post, put};
use ciwatcher_core::application::{
    ConnectionRepository, GetMonitoringSettings, NotifyRepositoryFailures,
    RepositorySelectionRepository, SecretStore, SettingsRepository, SourceRegistry,
};
use ciwatcher_core::contracts::{
    CONNECTIONS_HTTP_PATH, HEALTH_HTTP_PATH, REPOSITORIES_HTTP_PATH,
    REPOSITORY_SELECTIONS_HTTP_PATH, SETTINGS_HTTP_PATH, SOURCES_HTTP_PATH,
    SYNCHRONIZATION_HTTP_PATH, WORKFLOW_RUN_LOGS_HTTP_PATH, WORKFLOWS_HTTP_PATH,
};
use ciwatcher_core::source_data::SourceDataCache;
use handlers::{
    AppState, connect_source, disconnect_source, get_settings, health, list_repositories,
    list_sources, list_workflows, save_repository_selection, synchronization_status,
    synchronize_sources, update_settings, workflow_run_logs,
};
use std::sync::Arc;

pub struct ServerApplication {
    pub router: Router,
    pub synchronizer: ciwatcher_core::application::SynchronizeSources,
    pub settings: GetMonitoringSettings,
}

pub fn application(
    registry: SourceRegistry,
    connections: Arc<dyn ConnectionRepository>,
    secrets: Arc<dyn SecretStore>,
    repository_selections: Arc<dyn RepositorySelectionRepository>,
    source_data_cache: Arc<dyn SourceDataCache>,
    settings: Arc<dyn SettingsRepository>,
    failure_notifications: NotifyRepositoryFailures,
) -> ServerApplication {
    let state = AppState::new(
        registry,
        connections,
        secrets,
        repository_selections,
        source_data_cache,
        settings,
        failure_notifications,
    );
    let synchronizer = state.synchronizer();
    let settings = state.settings_reader();

    let router = Router::new()
        .route(HEALTH_HTTP_PATH, get(health))
        .route(WORKFLOWS_HTTP_PATH, get(list_workflows))
        .route(WORKFLOW_RUN_LOGS_HTTP_PATH, post(workflow_run_logs))
        .route(SETTINGS_HTTP_PATH, get(get_settings).put(update_settings))
        .route(
            SYNCHRONIZATION_HTTP_PATH,
            get(synchronization_status).post(synchronize_sources),
        )
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
        .with_state(state);
    ServerApplication {
        router,
        synchronizer,
        settings,
    }
}

pub fn router(
    registry: SourceRegistry,
    connections: Arc<dyn ConnectionRepository>,
    secrets: Arc<dyn SecretStore>,
    repository_selections: Arc<dyn RepositorySelectionRepository>,
    source_data_cache: Arc<dyn SourceDataCache>,
    settings: Arc<dyn SettingsRepository>,
    failure_notifications: NotifyRepositoryFailures,
) -> Router {
    application(
        registry,
        connections,
        secrets,
        repository_selections,
        source_data_cache,
        settings,
        failure_notifications,
    )
    .router
}

#[cfg(test)]
mod tests;
