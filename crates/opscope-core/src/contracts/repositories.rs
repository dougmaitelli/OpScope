use super::*;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum RepositoryVisibility {
    Public,
    Private,
}

impl From<DomainRepositoryVisibility> for RepositoryVisibility {
    fn from(visibility: DomainRepositoryVisibility) -> Self {
        match visibility {
            DomainRepositoryVisibility::Public => Self::Public,
            DomainRepositoryVisibility::Private => Self::Private,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySummary {
    pub id: String,
    pub owner: String,
    pub name: String,
    pub description: Option<String>,
    pub visibility: RepositoryVisibility,
    pub web_url: String,
    pub selected: bool,
}

impl From<RepositoryState> for RepositorySummary {
    fn from(state: RepositoryState) -> Self {
        let repository = state.repository;
        Self {
            id: repository.id,
            owner: repository.owner,
            name: repository.name,
            description: repository.description,
            visibility: repository.visibility.into(),
            web_url: repository.web_url,
            selected: state.selected,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySelectionSourceRequest {
    pub source_id: String,
    pub repository_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SaveRepositorySelectionRequest {
    pub sources: Vec<RepositorySelectionSourceRequest>,
}

impl SaveRepositorySelectionRequest {
    #[must_use]
    pub fn into_domain(self) -> Vec<SourceRepositorySelection> {
        self.sources
            .into_iter()
            .map(|source| SourceRepositorySelection {
                source_id: source.source_id,
                repository_ids: source.repository_ids,
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SaveRepositorySelectionResponse {
    pub selected_count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySelectionErrorResponse {
    pub message: String,
}

impl From<SaveRepositorySelectionFailure> for RepositorySelectionErrorResponse {
    fn from(failure: SaveRepositorySelectionFailure) -> Self {
        let message = match failure {
            SaveRepositorySelectionFailure::InvalidSelection => {
                "The repository selection is invalid. Refresh and try again."
            }
            SaveRepositorySelectionFailure::SourceNotConnected => {
                "A selected source is no longer connected. Refresh and try again."
            }
            SaveRepositorySelectionFailure::StorageUnavailable => {
                "The repository selection could not be stored."
            }
        };
        Self {
            message: message.to_owned(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySourceSummary {
    pub id: String,
    pub name: String,
    pub abbreviation: String,
    pub repositories: Vec<RepositorySummary>,
}

impl From<RepositoryCatalog> for RepositorySourceSummary {
    fn from(catalog: RepositoryCatalog) -> Self {
        Self {
            id: catalog.source.id,
            name: catalog.source.label,
            abbreviation: catalog.source.descriptor.abbreviation,
            repositories: catalog
                .repositories
                .into_iter()
                .map(RepositorySummary::from)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ListRepositoriesResponse {
    pub sources: Vec<RepositorySourceSummary>,
}

impl ListRepositoriesResponse {
    #[must_use]
    pub fn from_domain(catalogs: Vec<RepositoryCatalog>) -> Self {
        Self {
            sources: catalogs
                .into_iter()
                .map(RepositorySourceSummary::from)
                .collect(),
        }
    }
}
