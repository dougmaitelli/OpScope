use super::*;

impl GitHubClient {
    pub(super) async fn repositories(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
    ) -> Result<Vec<Repository>, ConnectionValidationFailure> {
        let mut repositories = Vec::new();
        for page in 1..=MAX_REPOSITORY_PAGES {
            let response = self
                .request(configuration, token, &["user", "repos"])?
                .query(&[("per_page", REPOSITORIES_PER_PAGE), ("page", page)])
                .send()
                .await
                .map_err(|_| ConnectionValidationFailure::ProviderUnavailable)?;
            if response.status() != StatusCode::OK {
                return Err(response_failure(&response));
            }

            let page_repositories = response
                .json::<Vec<GitHubRepository>>()
                .await
                .map_err(|_| ConnectionValidationFailure::UnexpectedResponse)?;
            let page_size = page_repositories.len();
            repositories.extend(map_available_repositories(page_repositories));
            if page_size < REPOSITORIES_PER_PAGE {
                return Ok(repositories);
            }
        }
        Err(ConnectionValidationFailure::UnexpectedResponse)
    }
}

#[derive(Deserialize)]
struct GitHubRepositoryOwner {
    login: String,
}

#[derive(Deserialize)]
pub(super) struct GitHubRepository {
    id: u64,
    owner: GitHubRepositoryOwner,
    name: String,
    description: Option<String>,
    private: bool,
    archived: bool,
    html_url: String,
}

impl From<GitHubRepository> for Repository {
    fn from(repository: GitHubRepository) -> Self {
        Self {
            id: repository.id.to_string(),
            owner: repository.owner.login,
            name: repository.name,
            description: repository.description,
            visibility: if repository.private {
                RepositoryVisibility::Private
            } else {
                RepositoryVisibility::Public
            },
            web_url: repository.html_url,
        }
    }
}

pub(super) fn map_available_repositories(repositories: Vec<GitHubRepository>) -> Vec<Repository> {
    repositories
        .into_iter()
        .filter(|repository| !repository.archived)
        .map(Repository::from)
        .collect()
}
