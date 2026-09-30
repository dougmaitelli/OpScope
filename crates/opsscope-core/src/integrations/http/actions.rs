use crate::application::{ActionFailure, ConnectionValidationFailure};
use reqwest::{Method, RequestBuilder, Response, StatusCode};
use serde::de::DeserializeOwned;

fn failure(response: &Response) -> ActionFailure {
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

pub(crate) async fn read<T: DeserializeOwned>(request: RequestBuilder) -> Result<T, ActionFailure> {
    let response = super::send(request).await?;
    if !response.status().is_success() {
        return Err(failure(&response));
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
        Err(failure(&response))
    }
}
