//! Minimal GitHub adapter used to validate a candidate connection.

use crate::application::{
    ConnectionValidationFailure, CredentialField, ProviderToken, SourceDescriptor, SourceModule,
    ValidatedAccount,
};
use async_trait::async_trait;
use reqwest::header::{ACCEPT, HeaderValue, USER_AGENT as USER_AGENT_HEADER};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use std::time::Duration;

const API_URL: &str = "https://api.github.com/user";
const API_VERSION: &str = "2026-03-10";
const ACCEPT_VALUE: &str = "application/vnd.github+json";
const USER_AGENT: &str = "CI-Watcher/0.1";

#[derive(Clone, Debug)]
pub struct GitHubClient {
    client: Client,
}

impl GitHubClient {
    pub fn new() -> Result<Self, ConnectionValidationFailure> {
        Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(USER_AGENT)
            .build()
            .map(|client| Self { client })
            .map_err(|_| ConnectionValidationFailure::ProviderUnavailable)
    }

    fn validation_request(&self, token: &ProviderToken) -> reqwest::RequestBuilder {
        self.client
            .get(API_URL)
            .bearer_auth(token.expose())
            .header(ACCEPT, ACCEPT_VALUE)
            .header(USER_AGENT_HEADER, USER_AGENT)
            .header("X-GitHub-Api-Version", API_VERSION)
    }
}

#[derive(Deserialize)]
struct GitHubUser {
    id: u64,
    login: String,
    name: Option<String>,
    html_url: String,
}

#[async_trait]
impl SourceModule for GitHubClient {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: "github".to_owned(),
            name: "GitHub".to_owned(),
            description: "Repositories, Actions workflows, and workflow runs".to_owned(),
            abbreviation: "GH".to_owned(),
            credential: CredentialField {
                label: "Fine-grained personal access token".to_owned(),
                placeholder: "github_pat_…".to_owned(),
                help: "Use a repository-scoped, read-only token.".to_owned(),
            },
        }
    }

    async fn validate(
        &self,
        token: &ProviderToken,
    ) -> Result<ValidatedAccount, ConnectionValidationFailure> {
        let response = self
            .validation_request(token)
            .send()
            .await
            .map_err(|_| ConnectionValidationFailure::ProviderUnavailable)?;

        match response.status() {
            StatusCode::OK => {
                let user = response
                    .json::<GitHubUser>()
                    .await
                    .map_err(|_| ConnectionValidationFailure::UnexpectedResponse)?;
                let name = user.name.unwrap_or_else(|| user.login.clone());
                Ok(ValidatedAccount {
                    external_id: user.id.to_string(),
                    name,
                    handle: Some(user.login),
                    profile_url: Some(user.html_url),
                })
            }
            StatusCode::UNAUTHORIZED => Err(ConnectionValidationFailure::InvalidCredentials),
            StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS
                if response
                    .headers()
                    .get("x-ratelimit-remaining")
                    .is_some_and(|value| value == HeaderValue::from_static("0"))
                    || response.headers().contains_key("retry-after")
                    || response.status() == StatusCode::TOO_MANY_REQUESTS =>
            {
                Err(ConnectionValidationFailure::RateLimited)
            }
            StatusCode::FORBIDDEN => Err(ConnectionValidationFailure::InvalidCredentials),
            status if status.is_server_error() => {
                Err(ConnectionValidationFailure::ProviderUnavailable)
            }
            _ => Err(ConnectionValidationFailure::UnexpectedResponse),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::AUTHORIZATION;

    #[test]
    fn validation_request_targets_only_github_with_required_headers()
    -> Result<(), Box<dyn std::error::Error>> {
        let client = GitHubClient::new()?;
        let token = ProviderToken::new("github_pat_test".to_owned());
        let request = client.validation_request(&token).build()?;

        assert_eq!(request.method(), reqwest::Method::GET);
        assert_eq!(request.url().as_str(), API_URL);
        assert_eq!(request.headers()[ACCEPT], ACCEPT_VALUE);
        assert_eq!(request.headers()["X-GitHub-Api-Version"], API_VERSION);
        assert_eq!(request.headers()[USER_AGENT_HEADER], USER_AGENT);
        assert_eq!(request.headers()[AUTHORIZATION], "Bearer github_pat_test");
        Ok(())
    }

    #[test]
    fn github_response_maps_to_a_non_secret_identity() -> Result<(), Box<dyn std::error::Error>> {
        let user: GitHubUser = serde_json::from_str(
            r#"{"id":42,"login":"octocat","name":"The Octocat","html_url":"https://github.com/octocat"}"#,
        )?;

        let validated = ValidatedAccount {
            external_id: user.id.to_string(),
            name: user.name.expect("fixture name"),
            handle: Some(user.login),
            profile_url: Some(user.html_url),
        };
        assert_eq!(validated.external_id, "42");
        assert_eq!(validated.handle.as_deref(), Some("octocat"));
        Ok(())
    }
}
