use super::{ConnectionValidationFailure, SourceCapability};
use crate::domain::ChangeRequestDetails;
use crate::source_data::{RefreshMode, SourceData, SourceDataFailure};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GetChangeRequestDetailsFailure {
    UnknownSource,
    SourceNotConnected,
    Unsupported,
    RepositoryNotFound,
    ChangeRequestNotFound,
    InvalidCredentials,
    PermissionDenied,
    RateLimited,
    ProviderUnavailable,
    UnexpectedResponse,
    StorageUnavailable,
}

impl Display for GetChangeRequestDetailsFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::UnknownSource => "source module is not registered",
            Self::SourceNotConnected => "source is not connected",
            Self::Unsupported => "source does not support change request details",
            Self::RepositoryNotFound => "repository is not available",
            Self::ChangeRequestNotFound => "change request is no longer available",
            Self::InvalidCredentials => "provider rejected the credential",
            Self::PermissionDenied => {
                "provider credential lacks permission to read change requests"
            }
            Self::RateLimited => "provider rate limit reached",
            Self::ProviderUnavailable => "provider unavailable",
            Self::UnexpectedResponse => "provider returned unexpected change request details",
            Self::StorageUnavailable => "stored source data could not be read",
        })
    }
}

impl Error for GetChangeRequestDetailsFailure {}

#[derive(Clone)]
pub struct GetChangeRequestDetails {
    source_data: Arc<dyn SourceData>,
}

impl GetChangeRequestDetails {
    #[must_use]
    pub fn new(source_data: Arc<dyn SourceData>) -> Self {
        Self { source_data }
    }

    pub async fn execute(
        &self,
        source_id: &str,
        repository_id: &str,
        number: u64,
    ) -> Result<ChangeRequestDetails, GetChangeRequestDetailsFailure> {
        let source = self
            .source_data
            .sources()
            .map_err(source_data_failure)?
            .into_iter()
            .find(|source| source.id == source_id)
            .ok_or(GetChangeRequestDetailsFailure::UnknownSource)?;
        if !source.descriptor.supports(SourceCapability::ChangeRequests) {
            return Err(GetChangeRequestDetailsFailure::Unsupported);
        }
        let repositories = self
            .source_data
            .repositories(source_id, RefreshMode::CacheFirst)
            .await
            .map_err(source_data_failure)?
            .ok_or(GetChangeRequestDetailsFailure::SourceNotConnected)?;
        let repository = repositories
            .into_iter()
            .find(|repository| repository.id == repository_id)
            .ok_or(GetChangeRequestDetailsFailure::RepositoryNotFound)?;
        self.source_data
            .change_request_details(source_id, &repository, number, RefreshMode::IfStale)
            .await
            .map_err(source_data_failure)?
            .ok_or(GetChangeRequestDetailsFailure::ChangeRequestNotFound)
    }
}

fn source_data_failure(failure: SourceDataFailure) -> GetChangeRequestDetailsFailure {
    match failure {
        SourceDataFailure::StorageUnavailable => GetChangeRequestDetailsFailure::StorageUnavailable,
        SourceDataFailure::Source(failure) => match failure {
            ConnectionValidationFailure::InvalidConfiguration => {
                GetChangeRequestDetailsFailure::UnexpectedResponse
            }
            ConnectionValidationFailure::InvalidCredentials => {
                GetChangeRequestDetailsFailure::InvalidCredentials
            }
            ConnectionValidationFailure::PermissionDenied => {
                GetChangeRequestDetailsFailure::PermissionDenied
            }
            ConnectionValidationFailure::RateLimited => GetChangeRequestDetailsFailure::RateLimited,
            ConnectionValidationFailure::ProviderUnavailable => {
                GetChangeRequestDetailsFailure::ProviderUnavailable
            }
            ConnectionValidationFailure::UnexpectedResponse => {
                GetChangeRequestDetailsFailure::UnexpectedResponse
            }
        },
    }
}
