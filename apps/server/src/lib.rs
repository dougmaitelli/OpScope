//! Self-hosted HTTP composition. Authentication is added before exposure.

pub mod auth;
mod handlers;
pub mod notifications;

use auth::{AUTH_LOGOUT_PATH, WebAuthentication, logout, require_authenticated_session};
use axum::Router;
use axum::http::StatusCode;
use axum::middleware;
use axum::routing::{any, get, post, put};
use handlers::{
    AppState, AppStateDependencies, change_request_details, connect_source, disconnect_source,
    get_settings, health, issue_details, list_activity, list_change_requests, list_issues,
    list_repositories, list_sources, list_workflows, save_repository_selection,
    synchronization_status, synchronize_sources, update_settings, update_status, workflow_run_logs,
};
use opsscope_core::application::{
    ActivityEventRepository, ConnectionRepository, GetMonitoringSettings, NotifyRepositoryFailures,
    RepositorySelectionRepository, SecretStore, SettingsRepository, SourceRegistry,
};
use opsscope_core::contracts::{
    ACTIVITY_HTTP_PATH, CHANGE_REQUEST_DETAILS_HTTP_PATH, CHANGE_REQUESTS_HTTP_PATH,
    CONNECTIONS_HTTP_PATH, HEALTH_HTTP_PATH, ISSUE_DETAILS_HTTP_PATH, ISSUES_HTTP_PATH,
    REPOSITORIES_HTTP_PATH, REPOSITORY_SELECTIONS_HTTP_PATH, SETTINGS_HTTP_PATH, SOURCES_HTTP_PATH,
    SYNCHRONIZATION_HTTP_PATH, UPDATE_STATUS_HTTP_PATH, WORKFLOW_RUN_LOGS_HTTP_PATH,
    WORKFLOWS_HTTP_PATH,
};
use opsscope_core::source_data::SourceDataCache;
use std::path::Path;
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};

pub struct ServerApplication {
    pub router: Router,
    pub synchronizer: opsscope_core::application::SynchronizeSources,
    pub settings: GetMonitoringSettings,
}

pub struct ServerDependencies {
    pub registry: SourceRegistry,
    pub connections: Arc<dyn ConnectionRepository>,
    pub secrets: Arc<dyn SecretStore>,
    pub repository_selections: Arc<dyn RepositorySelectionRepository>,
    pub source_data_cache: Arc<dyn SourceDataCache>,
    pub settings: Arc<dyn SettingsRepository>,
    pub activity_events: Arc<dyn ActivityEventRepository>,
    pub failure_notifications: NotifyRepositoryFailures,
    pub authentication: WebAuthentication,
}

pub fn application(dependencies: ServerDependencies) -> ServerApplication {
    let ServerDependencies {
        registry,
        connections,
        secrets,
        repository_selections,
        source_data_cache,
        settings,
        activity_events,
        failure_notifications,
        authentication,
    } = dependencies;
    let state = AppState::new(AppStateDependencies {
        registry,
        connections,
        secrets,
        repository_selections,
        source_data_cache,
        settings,
        activity_events,
        failure_notifications,
    });
    let synchronizer = state.synchronizer();
    let settings = state.settings_reader();

    let protected = Router::new()
        .route(UPDATE_STATUS_HTTP_PATH, get(update_status))
        .route(WORKFLOWS_HTTP_PATH, get(list_workflows))
        .route(ACTIVITY_HTTP_PATH, get(list_activity))
        .route(CHANGE_REQUESTS_HTTP_PATH, get(list_change_requests))
        .route(
            CHANGE_REQUEST_DETAILS_HTTP_PATH,
            post(change_request_details),
        )
        .route(ISSUES_HTTP_PATH, get(list_issues))
        .route(ISSUE_DETAILS_HTTP_PATH, post(issue_details))
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
        .route(AUTH_LOGOUT_PATH, post(logout))
        .route_layer(middleware::from_fn_with_state(
            authentication.clone(),
            require_authenticated_session,
        ));
    let router = Router::new()
        .route(HEALTH_HTTP_PATH, get(health))
        .merge(auth::routes())
        .merge(protected)
        .layer(axum::Extension(authentication))
        .with_state(state);
    ServerApplication {
        router,
        synchronizer,
        settings,
    }
}

pub fn router(dependencies: ServerDependencies) -> Router {
    application(dependencies).router
}

pub fn serve_web_application(router: Router, directory: impl AsRef<Path>) -> Router {
    let directory = directory.as_ref();
    let index = directory.join("index.html");
    router
        .route("/api", any(|| async { StatusCode::NOT_FOUND }))
        .route("/api/{*path}", any(|| async { StatusCode::NOT_FOUND }))
        .fallback_service(ServeDir::new(directory).fallback(ServeFile::new(index)))
}

#[cfg(test)]
mod tests;
