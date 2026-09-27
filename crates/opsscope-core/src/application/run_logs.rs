use super::WorkflowRunLogsFailure;
use crate::domain::{WorkflowRun, WorkflowRunLogs};
use crate::source_data::{RefreshMode, SourceData, SourceDataFailure};
use std::sync::Arc;

#[derive(Clone)]
pub struct GetWorkflowRunLogs {
    source_data: Arc<dyn SourceData>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedWorkflowRunLogs {
    pub run: WorkflowRun,
    pub logs: WorkflowRunLogs,
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
        attempt: Option<u64>,
    ) -> Result<ResolvedWorkflowRunLogs, WorkflowRunLogsFailure> {
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
        let cached_run = runs
            .runs
            .into_iter()
            .find(|run| run.id == run_id && attempt.is_none_or(|attempt| run.attempt == attempt));
        let mut run = match cached_run {
            Some(run) => run,
            None => self
                .source_data
                .workflow_run(source_id, &repository, run_id)
                .await
                .map_err(source_data_failure)?
                .ok_or(WorkflowRunLogsFailure::RunNotFound)?,
        };
        if let Some(attempt) = attempt {
            run.attempt = attempt;
        }

        let logs = self
            .source_data
            .workflow_run_logs(source_id, &repository, &run)
            .await?;
        Ok(ResolvedWorkflowRunLogs { run, logs })
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
