use serde::{Deserialize, Serialize};

/// Event switches affect delivery, not observation: disabled events still advance baselines.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct NotificationPreferences {
    pub workflow_failures: bool,
    pub dependabot_update_failures: bool,
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

impl Default for NotificationPreferences {
    fn default() -> Self {
        Self {
            workflow_failures: true,
            dependabot_update_failures: true,
            pull_request_opened: true,
            pull_request_review_requested: true,
            pull_request_changes_requested: true,
            pull_request_merged: true,
            pull_request_closed: true,
            issue_opened: true,
            issue_assigned: true,
            issue_reopened: true,
            issue_closed: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_preferences_preserve_choices_and_enable_dependabot_notifications() {
        let preferences: NotificationPreferences =
            serde_json::from_str(r#"{"workflow_failures":false,"issue_closed":false}"#).unwrap();
        assert!(!preferences.workflow_failures);
        assert!(!preferences.issue_closed);
        assert!(preferences.dependabot_update_failures);
    }
}
