use crate::application::{ActionFailure, ConnectionValidationFailure};
use reqwest::{Method, RequestBuilder, Response, StatusCode};
use serde::de::DeserializeOwned;

fn classify_failure(response: &Response) -> ActionFailure {
    match response.status() {
        StatusCode::NOT_FOUND => ActionFailure::NotFound,
        StatusCode::BAD_REQUEST | StatusCode::CONFLICT | StatusCode::UNPROCESSABLE_ENTITY => {
            ActionFailure::Conflict
        }
        StatusCode::METHOD_NOT_ALLOWED | StatusCode::NOT_IMPLEMENTED => ActionFailure::Unsupported,
        StatusCode::FORBIDDEN
            if response
                .headers()
                .get("x-ratelimit-remaining")
                .is_some_and(|v| v == "0")
                || response.headers().contains_key("retry-after") =>
        {
            ConnectionValidationFailure::RateLimited.into()
        }
        status => super::status_failure(status).into(),
    }
}

async fn failure(mut response: Response) -> ActionFailure {
    let classified = classify_failure(&response);
    if response.status() != StatusCode::FORBIDDEN
        || !matches!(
            classified,
            ActionFailure::Source(ConnectionValidationFailure::PermissionDenied)
        )
    {
        return classified;
    }
    if response.headers().get("x-github-sso").is_some_and(|value| {
        value
            .to_str()
            .is_ok_and(|value| value.starts_with("required"))
    }) {
        return ActionFailure::ProviderDenied {
            message: "GitHub requires SSO authorization for this token.".to_owned(),
        };
    }
    // Read only a bounded error payload, never the full provider response.
    let mut body = Vec::new();
    while let Ok(Some(chunk)) = response.chunk().await {
        if body.len() + chunk.len() > 16 * 1024 {
            return classified;
        }
        body.extend_from_slice(&chunk);
    }
    #[derive(serde::Deserialize)]
    struct ProviderError {
        message: String,
    }
    let Ok(error) = serde_json::from_slice::<ProviderError>(&body) else {
        return classified;
    };
    let message = error
        .message
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if message.is_empty() {
        return classified;
    }
    ActionFailure::ProviderDenied {
        message: message.chars().take(1000).collect(),
    }
}

pub(crate) async fn read<T: DeserializeOwned>(request: RequestBuilder) -> Result<T, ActionFailure> {
    let response = super::send(request).await?;
    if !response.status().is_success() {
        return Err(failure(response).await);
    }
    super::decode(response).await.map_err(Into::into)
}

/// Mutations are sent exactly once, never retried after ambiguous transport failures.
pub(crate) async fn write(
    request: RequestBuilder,
    method: Method,
    body: Option<serde_json::Value>,
) -> Result<(), ActionFailure> {
    write_response(request, method, body).await.map(|_| ())
}

pub(crate) async fn write_response(
    request: RequestBuilder,
    method: Method,
    body: Option<serde_json::Value>,
) -> Result<Response, ActionFailure> {
    let request = if let Some(body) = body {
        request.json(&body)
    } else {
        request
    };
    let (client, request) = request.build_split();
    let mut request = request
        .map_err(|_| ActionFailure::Source(ConnectionValidationFailure::InvalidConfiguration))?;
    *request.method_mut() = method;
    let response = client
        .execute(request)
        .await
        .map_err(|_| ActionFailure::OutcomeUnknown)?;
    if response.status().is_success() {
        Ok(response)
    } else if response.status().is_server_error() {
        Err(ActionFailure::OutcomeUnknown)
    } else {
        Err(failure(response).await)
    }
}
