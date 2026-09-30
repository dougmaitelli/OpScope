//! Adapters for external monitoring sources.

use crate::application::{ConnectionValidationFailure, SourceRegistry};
use std::sync::Arc;

pub mod bitbucket;
pub mod gitea;
pub mod github;
pub mod gitlab;
mod http;

const SERVER_URL_KEY: &str = "serverUrl";

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod tests;

/// Builds the compiled source catalog shared by desktop and server editions.
pub fn registered_sources() -> Result<SourceRegistry, ConnectionValidationFailure> {
    Ok(SourceRegistry::new(vec![
        Arc::new(github::GitHubClient::new()?),
        Arc::new(gitlab::GitLabClient::new()?),
        Arc::new(gitea::GiteaClient::new()?),
        Arc::new(bitbucket::BitbucketClient::new()?),
    ]))
}
