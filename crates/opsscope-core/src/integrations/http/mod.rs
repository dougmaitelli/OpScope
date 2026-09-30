//! Small transport helpers; provider routes and response models stay in their modules.
pub(crate) mod actions;

use super::SERVER_URL_KEY;
use crate::application::{
    ConfiguredSource, ConnectionConfiguration, ConnectionValidationFailure as Failure,
    WorkflowRunLogsFailure,
};
use reqwest::{Client, RequestBuilder, Response, StatusCode, Url};
use serde::de::DeserializeOwned;
use std::net::IpAddr;
use std::time::Duration;

pub(super) const PAGE_SIZE: usize = 50;
pub(super) const MAX_PAGES: usize = 100;
pub(super) const RUN_PAGES: usize = 2;
pub(super) const LOG_FILE_LIMIT: usize = 2 * 1024 * 1024;
pub(super) const LOG_TOTAL_LIMIT: usize = 10 * 1024 * 1024;
pub(super) const LOG_FILES_LIMIT: usize = 100;

pub(super) fn client() -> Result<Client, Failure> {
    Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("OpsScope/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| Failure::ProviderUnavailable)
}

pub(super) fn configure_server(
    configuration: &ConnectionConfiguration,
    default: &str,
) -> Result<ConfiguredSource, Failure> {
    if configuration.keys().any(|key| key != SERVER_URL_KEY) {
        return Err(Failure::InvalidConfiguration);
    }
    let value = configuration
        .get(SERVER_URL_KEY)
        .map_or(default, String::as_str)
        .trim();
    let value = if value.is_empty() { default } else { value };
    let url = Url::parse(value).map_err(|_| Failure::InvalidConfiguration)?;
    let host = url.host_str().ok_or(Failure::InvalidConfiguration)?;
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .trim_matches(['[', ']'])
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip.is_loopback());
    if (url.scheme() != "https" && !(url.scheme() == "http" && loopback))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || value.chars().any(char::is_control)
    {
        return Err(Failure::InvalidConfiguration);
    }
    let server = url.as_str().trim_end_matches('/').to_owned();
    Ok(ConfiguredSource {
        unique_key: server.clone(),
        label: url
            .port()
            .map_or_else(|| host.to_owned(), |port| format!("{host}:{port}")),
        configuration: [(SERVER_URL_KEY.to_owned(), server)].into_iter().collect(),
    })
}

pub(super) fn endpoint(base: &str, segments: &[&str]) -> Result<Url, Failure> {
    let mut url = Url::parse(base).map_err(|_| Failure::InvalidConfiguration)?;
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|()| Failure::InvalidConfiguration)?;
        path.pop_if_empty();
        for segment in segments {
            // URL's dot-segment normalization must never change the target route.
            if segment.is_empty() || matches!(*segment, "." | "..") {
                return Err(Failure::InvalidConfiguration);
            }
            path.push(segment);
        }
    }
    Ok(url)
}

pub(super) fn status_failure(status: StatusCode) -> Failure {
    match status {
        StatusCode::UNAUTHORIZED => Failure::InvalidCredentials,
        StatusCode::FORBIDDEN | StatusCode::NOT_FOUND => Failure::PermissionDenied,
        StatusCode::TOO_MANY_REQUESTS => Failure::RateLimited,
        status if status.is_server_error() => Failure::ProviderUnavailable,
        _ => Failure::UnexpectedResponse,
    }
}

pub(super) async fn send(request: RequestBuilder) -> Result<Response, Failure> {
    request
        .send()
        .await
        .map_err(|_| Failure::ProviderUnavailable)
}

pub(super) async fn json<T: DeserializeOwned>(request: RequestBuilder) -> Result<T, Failure> {
    let response = send(request).await?;
    if !response.status().is_success() {
        return Err(status_failure(response.status()));
    }
    decode(response).await
}

pub(super) async fn optional_json<T: DeserializeOwned>(
    request: RequestBuilder,
) -> Result<Option<T>, Failure> {
    let response = send(request).await?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err(status_failure(response.status()));
    }
    decode(response).await.map(Some)
}

pub(super) async fn decode<T: DeserializeOwned>(mut response: Response) -> Result<T, Failure> {
    const MAX_JSON_BYTES: usize = 16 * 1024 * 1024;
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| Failure::ProviderUnavailable)?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_JSON_BYTES {
            return Err(Failure::UnexpectedResponse);
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| Failure::UnexpectedResponse)
}

pub(super) fn log_failure(failure: Failure) -> WorkflowRunLogsFailure {
    match failure {
        Failure::InvalidCredentials => WorkflowRunLogsFailure::InvalidCredentials,
        Failure::PermissionDenied => WorkflowRunLogsFailure::PermissionDenied,
        Failure::RateLimited => WorkflowRunLogsFailure::RateLimited,
        Failure::ProviderUnavailable => WorkflowRunLogsFailure::ProviderUnavailable,
        Failure::InvalidConfiguration | Failure::UnexpectedResponse => {
            WorkflowRunLogsFailure::UnexpectedResponse
        }
    }
}

pub(super) async fn log_text(
    request: RequestBuilder,
    limit: usize,
) -> Result<(String, bool), WorkflowRunLogsFailure> {
    let response = send(request).await.map_err(log_failure)?;
    read_log(response, limit).await
}

pub(super) async fn read_log(
    mut response: Response,
    limit: usize,
) -> Result<(String, bool), WorkflowRunLogsFailure> {
    if response.status() == StatusCode::NOT_FOUND {
        return Err(WorkflowRunLogsFailure::LogsUnavailable);
    }
    if !response.status().is_success() {
        return Err(log_failure(status_failure(response.status())));
    }
    let mut bytes = Vec::new();
    let mut truncated = false;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| WorkflowRunLogsFailure::ProviderUnavailable)?
    {
        let remaining = limit.saturating_sub(bytes.len());
        bytes.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
        if chunk.len() > remaining {
            truncated = true;
            break;
        }
    }
    Ok((String::from_utf8_lossy(&bytes).into_owned(), truncated))
}

#[cfg(test)]
mod tests;
