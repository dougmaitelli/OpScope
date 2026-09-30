//! Application use cases and the ports they require.

mod actions;
mod activity;
pub use actions::*;
mod change_request_details;
mod change_requests;
mod connections;
mod issues;
mod notifications;
mod work_item_notifications;
pub use work_item_notifications::{
    NotifyWorkItems, WorkItemNotificationRepository, WorkItemNotificationState,
};
mod run_logs;
mod settings;
mod sources;
mod sync;
mod updates;
mod workflows;
pub use connections::*;
pub use sources::*;
mod repositories;
pub use repositories::*;

pub use notifications::{
    LatestRunNotificationState, NoopNotificationSink, Notification, NotificationDeliveryFailure,
    NotificationProcessingFailure, NotificationSeverity, NotificationSink,
    NotificationStateRepository, NotifyRepositoryFailures,
};
pub use run_logs::{GetWorkflowRunLogs, ResolvedWorkflowRunLogs};
pub use settings::{
    DEFAULT_RECENT_RUNS_PER_WORKFLOW, DEFAULT_SYNCHRONIZATION_INTERVAL_SECONDS,
    GetMonitoringSettings, MAX_RECENT_RUNS_PER_WORKFLOW, MAX_SYNCHRONIZATION_INTERVAL_SECONDS,
    MIN_RECENT_RUNS_PER_WORKFLOW, MIN_SYNCHRONIZATION_INTERVAL_SECONDS, MonitoringSettings,
    NotificationPreferences, SettingsFailure, SettingsRepository, UpdateMonitoringSettings,
};

pub use sync::{
    DEFAULT_SYNCHRONIZATION_INTERVAL, SynchronizationFailure, SynchronizationScope,
    SynchronizationStatus, SynchronizationSummary, SynchronizeSources,
};
pub use updates::{CheckForUpdates, ReleaseUpdate, UpdateCheckFailure};

pub use activity::{
    ActivityEventRepository, ChangeRequestActivityEvent, ChangeRequestActivityKind, ListActivity,
    TrackChangeRequestActivity,
};
pub use change_request_details::{GetChangeRequestDetails, GetChangeRequestDetailsFailure};
pub use change_requests::{
    ChangeRequestInventory, DiscoveredChangeRequest, ListChangeRequests, ListChangeRequestsFailure,
};
pub use issues::{
    DiscoveredIssue, GetIssueDetails, GetIssueDetailsFailure, IssueInventory, ListIssues,
    ListIssuesFailure,
};
pub use workflows::{DiscoveredWorkflow, ListWorkflows, ListWorkflowsFailure, WorkflowInventory};
