use ciwatcher_core::application::{DEFAULT_SYNCHRONIZATION_INTERVAL, SynchronizeSources};
use std::sync::Mutex;
use tauri::async_runtime::JoinHandle;

pub(crate) struct SynchronizationScheduler {
    task: Mutex<Option<JoinHandle<()>>>,
}

impl SynchronizationScheduler {
    pub(crate) fn start(synchronizer: SynchronizeSources) -> Self {
        let task = tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(DEFAULT_SYNCHRONIZATION_INTERVAL).await;
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
