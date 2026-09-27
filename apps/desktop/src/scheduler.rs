use ciwatcher_core::application::{
    DEFAULT_SYNCHRONIZATION_INTERVAL, GetMonitoringSettings, SynchronizeSources,
};
use std::sync::Mutex;
use tauri::async_runtime::JoinHandle;

pub(crate) struct SynchronizationScheduler {
    task: Mutex<Option<JoinHandle<()>>>,
}

impl SynchronizationScheduler {
    pub(crate) fn start(synchronizer: SynchronizeSources, settings: GetMonitoringSettings) -> Self {
        let task = tauri::async_runtime::spawn(async move {
            loop {
                let interval = settings
                    .execute()
                    .map(|settings| {
                        std::time::Duration::from_secs(settings.synchronization_interval_seconds)
                    })
                    .unwrap_or(DEFAULT_SYNCHRONIZATION_INTERVAL);
                tokio::time::sleep(interval).await;
                _ = synchronizer.execute().await;
            }
        });
        Self {
            task: Mutex::new(Some(task)),
        }
    }

    pub(crate) fn stop(&self) {
        let task = match self.task.lock() {
            Ok(mut task) => task.take(),
            Err(poisoned) => poisoned.into_inner().take(),
        };
        if let Some(task) = task {
            task.abort();
        }
    }
}

impl Drop for SynchronizationScheduler {
    fn drop(&mut self) {
        self.stop();
    }
}
