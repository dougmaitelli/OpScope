use crate::application::NotificationPreferences;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct NotificationPreferencesContract {
    pub workflow_failures: bool,
    pub pull_request_opened: bool,
    pub pull_request_review_requested: bool,
    pub pull_request_changes_requested: bool,
    pub pull_request_merged: bool,
    pub pull_request_closed: bool,
    pub issue_opened: bool,
    pub issue_assigned: bool,
    pub issue_reopened: bool,
    pub issue_closed: bool,
}

impl Default for NotificationPreferencesContract {
    fn default() -> Self {
        NotificationPreferences::default().into()
    }
}

impl From<NotificationPreferences> for NotificationPreferencesContract {
    fn from(value: NotificationPreferences) -> Self {
        Self {
            workflow_failures: value.workflow_failures,
            pull_request_opened: value.pull_request_opened,
            pull_request_review_requested: value.pull_request_review_requested,
            pull_request_changes_requested: value.pull_request_changes_requested,
            pull_request_merged: value.pull_request_merged,
            pull_request_closed: value.pull_request_closed,
            issue_opened: value.issue_opened,
            issue_assigned: value.issue_assigned,
            issue_reopened: value.issue_reopened,
            issue_closed: value.issue_closed,
        }
    }
}

impl From<NotificationPreferencesContract> for NotificationPreferences {
    fn from(value: NotificationPreferencesContract) -> Self {
        Self {
            workflow_failures: value.workflow_failures,
            pull_request_opened: value.pull_request_opened,
            pull_request_review_requested: value.pull_request_review_requested,
            pull_request_changes_requested: value.pull_request_changes_requested,
            pull_request_merged: value.pull_request_merged,
            pull_request_closed: value.pull_request_closed,
            issue_opened: value.issue_opened,
            issue_assigned: value.issue_assigned,
            issue_reopened: value.issue_reopened,
            issue_closed: value.issue_closed,
        }
    }
}
