use opsscope_core::application::{
    ActivityEventRepository, CheckForUpdates, ConnectSource, ConnectionRepository,
    DisconnectSource, GetChangeRequestDetails, GetMonitoringSettings, GetWorkflowRunLogs,
    ListActivity, ListChangeRequests, ListRepositories, ListSources, ListWorkflows,
    NotifyRepositoryFailures, RepositorySelectionRepository, SaveRepositorySelection, SecretStore,
    SettingsRepository, SourceRegistry, SynchronizeSources, TrackChangeRequestActivity,
    UpdateMonitoringSettings,
};
use opsscope_core::source_data::{ReadThroughSourceData, SourceDataCache, SourceDataCachePolicy};
use std::sync::Arc;

pub(crate) struct DesktopState {
    pub(crate) check_for_updates: CheckForUpdates,
    pub(crate) list_workflows: ListWorkflows,
    pub(crate) list_change_requests: ListChangeRequests,
    pub(crate) list_activity: ListActivity,
    pub(crate) change_request_details: GetChangeRequestDetails,
    pub(crate) workflow_run_logs: GetWorkflowRunLogs,
    pub(crate) connect_source: ConnectSource,
    pub(crate) list_sources: ListSources,
    pub(crate) list_repositories: ListRepositories,
    pub(crate) save_repository_selection: SaveRepositorySelection,
    pub(crate) disconnect_source: DisconnectSource,
    pub(crate) synchronize_sources: SynchronizeSources,
    pub(crate) get_settings: GetMonitoringSettings,
    pub(crate) update_settings: UpdateMonitoringSettings,
}

pub(crate) struct DesktopStateDependencies {
    pub sources: SourceRegistry,
    pub connections: Arc<dyn ConnectionRepository>,
    pub secrets: Arc<dyn SecretStore>,
    pub repository_selections: Arc<dyn RepositorySelectionRepository>,
    pub source_data_cache: Arc<dyn SourceDataCache>,
    pub settings: Arc<dyn SettingsRepository>,
    pub activity_events: Arc<dyn ActivityEventRepository>,
    pub failure_notifications: NotifyRepositoryFailures,
}

impl DesktopState {
    pub(crate) fn new(dependencies: DesktopStateDependencies) -> Self {
        let DesktopStateDependencies {
            sources,
            connections,
            secrets,
            repository_selections,
            source_data_cache,
            settings,
            activity_events,
            failure_notifications,
        } = dependencies;
        let source_data = Arc::new(ReadThroughSourceData::cached(
            sources.clone(),
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
            list_activity: ListActivity::new(
                activity_events.clone(),
                repository_selections.clone(),
            ),
            change_request_details: GetChangeRequestDetails::new(source_data.clone()),
            workflow_run_logs: GetWorkflowRunLogs::new(source_data.clone()),
            connect_source: ConnectSource::new(
                sources.clone(),
                connections.clone(),
                secrets.clone(),
            ),
            list_sources: ListSources::new(sources.clone(), connections.clone()),
            list_repositories: ListRepositories::new(
                source_data.clone(),
                repository_selections.clone(),
            ),
            synchronize_sources: SynchronizeSources::new(
                source_data,
                repository_selections.clone(),
                failure_notifications,
                TrackChangeRequestActivity::new(activity_events),
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
}
