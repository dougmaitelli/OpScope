use ciwatcher_core::application::{
    ConnectSource, ConnectionRepository, DisconnectSource, ListRepositories, ListSources,
    ListWorkflows, RepositorySelectionRepository, SaveRepositorySelection, SecretStore,
    SourceRegistry,
};
use ciwatcher_core::source_data::{ReadThroughSourceData, SourceDataCache, SourceDataCachePolicy};
use std::sync::Arc;

pub(crate) struct DesktopState {
    pub(crate) list_workflows: ListWorkflows,
    pub(crate) connect_source: ConnectSource,
    pub(crate) list_sources: ListSources,
    pub(crate) list_repositories: ListRepositories,
    pub(crate) save_repository_selection: SaveRepositorySelection,
    pub(crate) disconnect_source: DisconnectSource,
}

impl DesktopState {
    pub(crate) fn new(
        sources: SourceRegistry,
        connections: Arc<dyn ConnectionRepository>,
        secrets: Arc<dyn SecretStore>,
        repository_selections: Arc<dyn RepositorySelectionRepository>,
        source_data_cache: Arc<dyn SourceDataCache>,
    ) -> Self {
        let source_data = Arc::new(ReadThroughSourceData::cached(
            sources.clone(),
            connections.clone(),
            secrets.clone(),
            source_data_cache,
            SourceDataCachePolicy::default(),
        ));
        Self {
            list_workflows: ListWorkflows::new(source_data.clone(), repository_selections.clone()),
            connect_source: ConnectSource::new(
                sources.clone(),
                connections.clone(),
                secrets.clone(),
            ),
            list_sources: ListSources::new(sources.clone(), connections.clone()),
            list_repositories: ListRepositories::new(source_data, repository_selections.clone()),
            save_repository_selection: SaveRepositorySelection::new(
                sources.clone(),
                connections.clone(),
                repository_selections,
            ),
            disconnect_source: DisconnectSource::new(sources, connections, secrets),
        }
    }
}
