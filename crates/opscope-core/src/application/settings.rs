use super::PersistenceFailure;
mod notifications;
pub use notifications::NotificationPreferences;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

pub const DEFAULT_SYNCHRONIZATION_INTERVAL_SECONDS: u64 = 60;
pub const MIN_SYNCHRONIZATION_INTERVAL_SECONDS: u64 = 30;
pub const MAX_SYNCHRONIZATION_INTERVAL_SECONDS: u64 = 3_600;
pub const DEFAULT_RECENT_RUNS_PER_WORKFLOW: usize = 10;
pub const MIN_RECENT_RUNS_PER_WORKFLOW: usize = 1;
pub const MAX_RECENT_RUNS_PER_WORKFLOW: usize = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MonitoringSettings {
    pub pull_requests_enabled: bool,
    pub issues_enabled: bool,
    pub notifications: NotificationPreferences,
    pub only_my_work: bool,
    pub synchronization_interval_seconds: u64,
    pub recent_runs_per_workflow: usize,
}

impl Default for MonitoringSettings {
    fn default() -> Self {
        Self {
            pull_requests_enabled: true,
            issues_enabled: true,
            notifications: NotificationPreferences::default(),
            only_my_work: false,
            synchronization_interval_seconds: DEFAULT_SYNCHRONIZATION_INTERVAL_SECONDS,
            recent_runs_per_workflow: DEFAULT_RECENT_RUNS_PER_WORKFLOW,
        }
    }
}

impl MonitoringSettings {
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.synchronization_interval_seconds >= MIN_SYNCHRONIZATION_INTERVAL_SECONDS
            && self.synchronization_interval_seconds <= MAX_SYNCHRONIZATION_INTERVAL_SECONDS
            && self.recent_runs_per_workflow >= MIN_RECENT_RUNS_PER_WORKFLOW
            && self.recent_runs_per_workflow <= MAX_RECENT_RUNS_PER_WORKFLOW
    }
}

pub trait SettingsRepository: Send + Sync {
    fn load_settings(&self) -> Result<MonitoringSettings, PersistenceFailure>;
    fn save_settings(&self, settings: MonitoringSettings) -> Result<(), PersistenceFailure>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsFailure {
    InvalidSettings,
    StorageUnavailable,
}

impl Display for SettingsFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSettings => "monitoring settings are outside the allowed range",
            Self::StorageUnavailable => "monitoring settings storage unavailable",
        })
    }
}

impl Error for SettingsFailure {}

#[derive(Clone)]
pub struct GetMonitoringSettings {
    settings: Arc<dyn SettingsRepository>,
}

impl GetMonitoringSettings {
    #[must_use]
    pub fn new(settings: Arc<dyn SettingsRepository>) -> Self {
        Self {
            settings,
        }
    }

    pub fn execute(&self) -> Result<MonitoringSettings, SettingsFailure> {
        self.settings
            .load_settings()
            .map_err(|_| SettingsFailure::StorageUnavailable)
    }
}

#[derive(Clone)]
pub struct UpdateMonitoringSettings {
    settings: Arc<dyn SettingsRepository>,
}

impl UpdateMonitoringSettings {
    #[must_use]
    pub fn new(settings: Arc<dyn SettingsRepository>) -> Self {
        Self {
            settings,
        }
    }

    pub fn execute(
        &self,
        settings: MonitoringSettings,
    ) -> Result<MonitoringSettings, SettingsFailure> {
        if !settings.is_valid() {
            return Err(SettingsFailure::InvalidSettings);
        }
        self.settings
            .save_settings(settings)
            .map_err(|_| SettingsFailure::StorageUnavailable)?;
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MemorySettings(Mutex<MonitoringSettings>);

    impl SettingsRepository for MemorySettings {
        fn load_settings(&self) -> Result<MonitoringSettings, PersistenceFailure> {
            self.0
                .lock()
                .map(|settings| *settings)
                .map_err(|_| PersistenceFailure)
        }

        fn save_settings(&self, settings: MonitoringSettings) -> Result<(), PersistenceFailure> {
            self.0
                .lock()
                .map(|mut current| *current = settings)
                .map_err(|_| PersistenceFailure)
        }
    }

    #[test]
    fn update_rejects_values_outside_safe_bounds() {
        let repository = Arc::new(MemorySettings(Mutex::new(MonitoringSettings::default())));
        let update = UpdateMonitoringSettings::new(repository);

        assert_eq!(
            update.execute(MonitoringSettings {
                pull_requests_enabled: true,
                issues_enabled: true,
                notifications: NotificationPreferences::default(),
                only_my_work: false,
                synchronization_interval_seconds: 1,
                recent_runs_per_workflow: 10,
            }),
            Err(SettingsFailure::InvalidSettings)
        );
    }
}
