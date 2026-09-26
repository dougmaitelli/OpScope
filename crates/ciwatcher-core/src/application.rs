//! Application use cases and the ports they require.

use crate::domain::Monitor;
use async_trait::async_trait;
use std::error::Error;
use std::fmt::{Display, Formatter};

/// A failure reported by an external monitoring source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceError {
    message: String,
}

impl SourceError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for SourceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for SourceError {}

/// Port implemented by provider integrations.
#[async_trait]
pub trait MonitorSource: Send + Sync {
    async fn list_monitors(&self) -> Result<Vec<Monitor>, SourceError>;
}

/// Lists monitors without knowing which provider supplies them.
#[derive(Clone, Debug)]
pub struct ListMonitors<S> {
    source: S,
}

impl<S> ListMonitors<S>
where
    S: MonitorSource,
{
    #[must_use]
    pub fn new(source: S) -> Self {
        Self { source }
    }

    pub async fn execute(&self) -> Result<Vec<Monitor>, SourceError> {
        self.source.list_monitors().await
    }
}
