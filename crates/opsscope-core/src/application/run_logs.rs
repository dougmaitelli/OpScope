use super::WorkflowRunLogsFailure;
use crate::domain::WorkflowRunLogs;
use crate::source_data::{RefreshMode, SourceData, SourceDataFailure};
use std::sync::Arc;

#[derive(Clone)]
pub struct GetWorkflowRunLogs {
    source_data: Arc<dyn SourceData>,
}

impl GetWorkflowRunLogs {
    #[must_use]
    pub fn new(source_data: Arc<dyn SourceData>) -> Self {
        Self { source_data }
    }

    pub async fn execute(
        &self,
        source_id: &str,
        repository_id: &str,
        run_id: &str,
        attempt: u64,
    ) -> Result<WorkflowRunLogs, WorkflowRunLogsFailure> {
        if !self
            .source_data
            .sources()
            .map_err(source_data_failure)?
            .iter()
            .any(|source| source.id == source_id)
        {
            return Err(WorkflowRunLogsFailure::UnknownSource);
        }

        let repositories = self
            .source_data
            .repositories(source_id, RefreshMode::CacheFirst)
            .await
            .map_err(source_data_failure)?
            .ok_or(WorkflowRunLogsFailure::SourceNotConnected)?;
        let repository = repositories
            .into_iter()
            .find(|repository| repository.id == repository_id)
            .ok_or(WorkflowRunLogsFailure::RepositoryNotFound)?;
        let runs = self
            .source_data
            .workflow_runs(source_id, &repository, RefreshMode::CacheFirst)
            .await
            .map_err(source_data_failure)?;
        let run = runs
            .runs
            .into_iter()
            .find(|run| run.id == run_id && run.attempt == attempt)
            .ok_or(WorkflowRunLogsFailure::RunNotFound)?;

        self.source_data
            .workflow_run_logs(source_id, &repository, &run)
            .await
    }
}

fn source_data_failure(failure: SourceDataFailure) -> WorkflowRunLogsFailure {
    match failure {
        SourceDataFailure::StorageUnavailable => WorkflowRunLogsFailure::StorageUnavailable,
        SourceDataFailure::Source(failure) => match failure {
            super::ConnectionValidationFailure::InvalidConfiguration => {
                WorkflowRunLogsFailure::UnexpectedResponse
            }
            super::ConnectionValidationFailure::InvalidCredentials => {
                WorkflowRunLogsFailure::InvalidCredentials
            }
            super::ConnectionValidationFailure::PermissionDenied => {
                WorkflowRunLogsFailure::PermissionDenied
            }
            super::ConnectionValidationFailure::RateLimited => WorkflowRunLogsFailure::RateLimited,
            super::ConnectionValidationFailure::ProviderUnavailable => {
                WorkflowRunLogsFailure::ProviderUnavailable
            }
            super::ConnectionValidationFailure::UnexpectedResponse => {
                WorkflowRunLogsFailure::UnexpectedResponse
            }
        },
    }
}
