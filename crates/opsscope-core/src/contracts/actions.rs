use super::*;
pub use crate::application::{ActionOptions, ActionTarget, AvailableAction, SourceAction};

pub const ACTION_OPTIONS_HTTP_PATH: &str = "/api/action-options";
pub const EXECUTE_ACTION_HTTP_PATH: &str = "/api/actions";
pub const ACTION_OPTIONS_DESKTOP_COMMAND: &str = "action_options";
pub const EXECUTE_ACTION_DESKTOP_COMMAND: &str = "execute_action";

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ActionOptionsRequest {
    pub source_id: String,
    pub repository_id: String,
    pub target: ActionTarget,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteActionRequest {
    pub source_id: String,
    pub repository_id: String,
    pub target: ActionTarget,
    pub action: SourceAction,
    pub revision: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteActionResponse {
    pub accepted: bool,
    pub run: Option<WorkflowRunSummary>,
    pub change_request: Option<ChangeRequestSummary>,
    pub details: Option<ChangeRequestDetailsResponse>,
}

impl From<crate::application::ActionRefresh> for ExecuteActionResponse {
    fn from(refresh: crate::application::ActionRefresh) -> Self {
        Self {
            accepted: true,
            run: refresh.run.map(Into::into),
            change_request: refresh.change_request.map(Into::into),
            details: refresh.details.map(Into::into),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
pub struct ActionErrorResponse {
    pub code: String,
    pub message: String,
}

impl From<crate::application::ActionFailure> for ActionErrorResponse {
    fn from(failure: crate::application::ActionFailure) -> Self {
        use crate::application::ActionFailure;
        let code = match failure {
            ActionFailure::NotFound => "action_resource_not_found",
            ActionFailure::Unsupported => "action_unsupported",
            ActionFailure::Conflict => "action_conflict",
            ActionFailure::Busy => "action_busy",
            ActionFailure::StorageUnavailable => "action_storage_unavailable",
            ActionFailure::Source(_)
            | ActionFailure::ProviderDenied {
                ..
            } => "action_provider_error",
            ActionFailure::OutcomeUnknown => "action_outcome_unknown",
        };
        Self {
            code: code.into(),
            message: failure.to_string(),
        }
    }
}
