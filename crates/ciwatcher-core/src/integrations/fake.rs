//! Deterministic integration used to exercise application boundaries.

use crate::application::{MonitorSource, SourceError};
use crate::domain::{Monitor, MonitorStatus};
use async_trait::async_trait;

#[derive(Clone, Copy, Debug, Default)]
pub struct FakeMonitorSource;

#[async_trait]
impl MonitorSource for FakeMonitorSource {
    async fn list_monitors(&self) -> Result<Vec<Monitor>, SourceError> {
        Ok(vec![Monitor {
            id: "fake:workflow:build".to_owned(),
            name: "Synthetic build".to_owned(),
            status: MonitorStatus::Passing,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn returns_deterministic_monitor_data() -> Result<(), SourceError> {
        let monitors = FakeMonitorSource.list_monitors().await?;

        assert_eq!(monitors.len(), 1);
        assert_eq!(monitors[0].name, "Synthetic build");
        Ok(())
    }
}
