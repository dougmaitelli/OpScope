//! Development-only HTTP composition. Authentication is added before exposure.

mod handlers;

use axum::Router;
use axum::routing::{get, post, put};
use ciwatcher_core::application::{
    ConnectionRepository, RepositorySelectionRepository, SecretStore, SourceRegistry,
};
use ciwatcher_core::contracts::{
    CONNECTIONS_HTTP_PATH, HEALTH_HTTP_PATH, REPOSITORIES_HTTP_PATH,
    REPOSITORY_SELECTIONS_HTTP_PATH, SOURCES_HTTP_PATH, WORKFLOWS_HTTP_PATH,
};
use handlers::{
    AppState, connect_source, disconnect_source, health, list_repositories, list_sources,
    list_workflows, save_repository_selection,
};
use std::sync::Arc;

pub fn router(
    registry: SourceRegistry,
    connections: Arc<dyn ConnectionRepository>,
    secrets: Arc<dyn SecretStore>,
    repository_selections: Arc<dyn RepositorySelectionRepository>,
) -> Router {
    let state = AppState::new(registry, connections, secrets, repository_selections);

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

#[cfg(test)]
mod tests;
