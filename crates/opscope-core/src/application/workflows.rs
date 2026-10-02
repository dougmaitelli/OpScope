use super::{
    ConnectedSource, ConnectionValidationFailure, RepositorySelectionRepository, SettingsRepository,
};
use crate::domain::{Repository, Workflow, WorkflowRun};
use crate::source_data::{RefreshMode, SourceData, SourceDataFailure};
use std::collections::HashSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredWorkflow {
    pub source: ConnectedSource,
    pub repository: Repository,
    pub workflow: Workflow,
    pub runs: Vec<WorkflowRun>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowInventory {
    pub selected_repository_count: usize,
    pub workflows: Vec<DiscoveredWorkflow>,
    pub last_attempted_at: Option<u64>,
    pub last_successful_at: Option<u64>,
    pub stale: bool,
    pub sync_error: Option<ConnectionValidationFailure>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListWorkflowsFailure {
    Source(ConnectionValidationFailure),
    StorageUnavailable,
}

impl Display for ListWorkflowsFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(failure) => Display::fmt(failure, formatter),
            Self::StorageUnavailable => {
                formatter.write_str("workflow discovery storage unavailable")
            }
        }
    }
}

impl Error for ListWorkflowsFailure {}

#[derive(Clone)]
pub struct ListWorkflows {
    source_data: Arc<dyn SourceData>,
    selections: Arc<dyn RepositorySelectionRepository>,
    settings: Arc<dyn SettingsRepository>,
}

impl ListWorkflows {
    #[must_use]
    pub fn new(
        source_data: Arc<dyn SourceData>,
        selections: Arc<dyn RepositorySelectionRepository>,
        settings: Arc<dyn SettingsRepository>,
    ) -> Self {
        Self {
            source_data,
            selections,
            settings,
        }
    }

    pub async fn execute(&self) -> Result<WorkflowInventory, ListWorkflowsFailure> {
        let settings = self
            .settings
            .load_settings()
            .map_err(|_| ListWorkflowsFailure::StorageUnavailable)?;
        let recent_runs_per_workflow = settings.recent_runs_per_workflow;
        let selections = self
            .selections
            .list()
            .map_err(|_| ListWorkflowsFailure::StorageUnavailable)?;
        let selected_repository_count = selections.len();
        let mut workflows = Vec::new();
        let mut last_attempted_at = None;
        let mut last_successful_at = None;
        let mut stale = false;
        let mut sync_error = None;

        for source in self.source_data.sources().map_err(list_workflows_failure)? {
            let selected_ids = selections
                .iter()
                .filter(|selection| selection.source_id == source.id)
                .map(|selection| selection.repository_id.clone())
                .collect::<HashSet<_>>();
            if selected_ids.is_empty() {
                continue;
            }

            let repositories = self
                .source_data
                .repositories(&source.id, RefreshMode::CacheFirst)
                .await
                .map_err(list_workflows_failure)?
                .ok_or(ListWorkflowsFailure::StorageUnavailable)?;

            for repository in repositories
                .into_iter()
                .filter(|repository| selected_ids.contains(&repository.id))
            {
                let repository_workflows = self
                    .source_data
                    .workflows(&source.id, &repository, RefreshMode::CacheFirst)
                    .await
                    .map_err(list_workflows_failure)?;
                let repository_runs = self
                    .source_data
                    .workflow_runs(&source.id, &repository, RefreshMode::CacheFirst)
                    .await
                    .map_err(list_workflows_failure)?;
                last_attempted_at = Some(
                    last_attempted_at.map_or(repository_runs.last_attempted_at, |current: u64| {
                        current.max(repository_runs.last_attempted_at)
                    }),
                );
                last_successful_at = Some(
                    last_successful_at
                        .map_or(repository_runs.last_successful_at, |current: u64| {
                            current.min(repository_runs.last_successful_at)
                        }),
                );
                stale |= repository_runs.stale;
                sync_error = sync_error.or(repository_runs.error);
                workflows.extend(repository_workflows.into_iter().map(|workflow| {
                    let runs = repository_runs
                        .runs
                        .iter()
                        .filter(|run| run.workflow_id == workflow.id)
                        .filter(|run| {
                            !settings.only_my_work
                                || run.relationships.evaluate(&source.account_id).matches()
                        })
                        .take(recent_runs_per_workflow)
                        .cloned()
                        .collect();
                    DiscoveredWorkflow {
                        source: source.clone(),
                        repository: repository.clone(),
                        workflow,
                        runs,
                    }
                }));
            }
        }

        workflows.retain(|workflow| !settings.only_my_work || !workflow.runs.is_empty());
        Ok(WorkflowInventory {
            selected_repository_count,
            workflows,
            last_attempted_at,
            last_successful_at,
            stale,
            sync_error,
        })
    }
}

fn list_workflows_failure(failure: SourceDataFailure) -> ListWorkflowsFailure {
    match failure {
        SourceDataFailure::Source(failure) => ListWorkflowsFailure::Source(failure),
        SourceDataFailure::StorageUnavailable => ListWorkflowsFailure::StorageUnavailable,
    }
}
