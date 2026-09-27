use async_trait::async_trait;
use ciwatcher_core::application::{
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

pub const APPRISE_URL_ENVIRONMENT_VARIABLE: &str = "CIWATCHER_APPRISE_URL";

#[derive(Debug)]
pub struct AppriseConfigurationError;

impl Display for AppriseConfigurationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(
            "CIWATCHER_APPRISE_URL must be a valid HTTP or HTTPS Apprise notification endpoint",
        )
    }
}

impl Error for AppriseConfigurationError {}

pub fn notification_sink_from_environment()
-> Result<Arc<dyn NotificationSink>, AppriseConfigurationError> {
    let endpoint = match env::var(APPRISE_URL_ENVIRONMENT_VARIABLE) {
        Ok(endpoint) => endpoint,
        Err(env::VarError::NotPresent) => return Ok(Arc::new(NoopNotificationSink)),
        Err(env::VarError::NotUnicode(_)) => return Err(AppriseConfigurationError),
    };
    Ok(Arc::new(AppriseNotificationSink::new(&endpoint)?))
}

pub struct AppriseNotificationSink {
    client: Client,
    endpoint: Url,
}

impl AppriseNotificationSink {
    pub fn new(endpoint: &str) -> Result<Self, AppriseConfigurationError> {
        let endpoint = Url::parse(endpoint).map_err(|_| AppriseConfigurationError)?;
        if !matches!(endpoint.scheme(), "http" | "https") {
            return Err(AppriseConfigurationError);
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| AppriseConfigurationError)?;
        Ok(Self { client, endpoint })
    }
}

#[derive(Serialize)]
struct AppriseNotification<'a> {
    title: &'a str,
    body: &'a str,
    #[serde(rename = "type")]
    kind: &'static str,
    format: &'static str,
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
            })
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map(|_| ())
            .map_err(|_| NotificationDeliveryFailure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_an_http_endpoint() {
        assert!(AppriseNotificationSink::new("https://apprise.test/notify/ciwatcher").is_ok());
        assert!(AppriseNotificationSink::new("ftp://apprise.test/notify").is_err());
        assert!(AppriseNotificationSink::new("not a url").is_err());
    }

    #[test]
    fn serializes_the_apprise_failure_contract() {
        let payload = AppriseNotification {
            title: "Workflow failed",
            body: "Build",
            kind: "failure",
            format: "text",
        };

        assert_eq!(
            serde_json::to_value(payload).expect("payload serializes"),
            serde_json::json!({
                "title": "Workflow failed",
                "body": "Build",
                "type": "failure",
                "format": "text"
            })
        );
    }
}
