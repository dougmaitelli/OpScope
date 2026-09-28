use async_trait::async_trait;
use opsscope_core::application::{
    NoopNotificationSink, Notification, NotificationDeliveryFailure, NotificationSeverity,
    NotificationSink,
};
use reqwest::{Client, Url};
use serde::Serialize;
use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::time::Duration;

pub const APPRISE_URL_ENVIRONMENT_VARIABLE: &str = "OPSSCOPE_APPRISE_URL";
pub const APPRISE_TAGS_ENVIRONMENT_VARIABLE: &str = "OPSSCOPE_APPRISE_TAGS";

const INVALID_ENDPOINT: AppriseConfigurationError = AppriseConfigurationError(
    "OPSSCOPE_APPRISE_URL must be a valid HTTP or HTTPS Apprise notification endpoint",
);

#[derive(Debug)]
pub struct AppriseConfigurationError(&'static str);

impl Display for AppriseConfigurationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for AppriseConfigurationError {}

pub fn notification_sink_from_environment()
-> Result<Arc<dyn NotificationSink>, AppriseConfigurationError> {
    let endpoint = match env::var(APPRISE_URL_ENVIRONMENT_VARIABLE) {
        Ok(endpoint) => endpoint,
        Err(env::VarError::NotPresent) => return Ok(Arc::new(NoopNotificationSink)),
        Err(env::VarError::NotUnicode(_)) => return Err(INVALID_ENDPOINT),
    };
    let tags = match env::var(APPRISE_TAGS_ENVIRONMENT_VARIABLE) {
        Ok(tags) => tags,
        Err(env::VarError::NotPresent) => String::new(),
        Err(env::VarError::NotUnicode(_)) => {
            return Err(AppriseConfigurationError(
                "OPSSCOPE_APPRISE_TAGS must contain valid Unicode",
            ));
        }
    };
    Ok(Arc::new(
        AppriseNotificationSink::new(&endpoint)?.with_tags(&tags),
    ))
}

pub struct AppriseNotificationSink {
    client: Client,
    endpoint: Url,
    tags: Vec<String>,
}

impl AppriseNotificationSink {
    pub fn new(endpoint: &str) -> Result<Self, AppriseConfigurationError> {
        let endpoint = Url::parse(endpoint).map_err(|_| INVALID_ENDPOINT)?;
        if !matches!(endpoint.scheme(), "http" | "https") {
            return Err(INVALID_ENDPOINT);
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| INVALID_ENDPOINT)?;
        Ok(Self {
            client,
            endpoint,
            tags: Vec::new(),
        })
    }

    #[must_use]
    pub fn with_tags(mut self, tags: &str) -> Self {
        self.tags = tags
            .split(',')
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .map(str::to_owned)
            .collect();
        self
    }
}

#[derive(Serialize)]
struct AppriseNotification<'a> {
    title: &'a str,
    body: &'a str,
    #[serde(rename = "type")]
    kind: &'static str,
    format: &'static str,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    tag: &'a [String],
}

#[async_trait]
impl NotificationSink for AppriseNotificationSink {
    async fn send(&self, notification: &Notification) -> Result<(), NotificationDeliveryFailure> {
        let kind = match notification.severity {
            NotificationSeverity::Failure => "failure",
        };
        self.client
            .post(self.endpoint.clone())
            .json(&AppriseNotification {
                title: &notification.title,
                body: &notification.body,
                kind,
                format: "text",
                tag: &self.tags,
            })
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map(|_| ())
            .map_err(|_| NotificationDeliveryFailure)
    }
}

#[cfg(test)]
mod tests;
