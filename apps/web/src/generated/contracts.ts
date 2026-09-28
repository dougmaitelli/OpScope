// Generated from crates/opsscope-core/src/contracts.rs. Do not edit.

export type HealthResponse = { status: string, service: string, contractVersion: number, };

export type UpdateStatusResponse = { currentVersion: string, latestVersion: string, releaseUrl: string, updateAvailable: boolean, };

export type ConnectSourceRequest = { sourceId: string, connectionId: string | null, configuration: { [key in string]: string }, credential: string, };

export type ConnectionSummary = { id: string, label: string, configuration: { [key in string]: string }, externalId: string, name: string, handle: string | null, profileUrl: string | null, credentialStored: boolean, };

export type CredentialFieldSummary = { label: string, placeholder: string, help: string, };

export type ConnectionFieldSummary = { key: string, label: string, placeholder: string, help: string, defaultValue: string, };

export type SourceCapability = "workflows" | "changeRequests" | "issues";

export type SourceSummary = { id: string, name: string, description: string, abbreviation: string, capabilities: Array<SourceCapability>, credential: CredentialFieldSummary, connectionFields: Array<ConnectionFieldSummary>, connections: Array<ConnectionSummary>, };

export type ListSourcesResponse = { sources: Array<SourceSummary>, };

export type DisconnectSourceRequest = { connectionId: string, };

export type DisconnectSourceResponse = { disconnected: boolean, };

export type ConnectionValidationErrorCode = "invalidConfiguration" | "invalidCredentials" | "permissionDenied" | "rateLimited" | "providerUnavailable" | "unexpectedResponse" | "storageUnavailable" | "unknownSource" | "unknownConnection" | "duplicateConnection";

export type ConnectionValidationErrorResponse = { code: ConnectionValidationErrorCode, message: string, };

export type RepositoryVisibility = "public" | "private";

export type RepositorySummary = { id: string, owner: string, name: string, description: string | null, visibility: RepositoryVisibility, webUrl: string, selected: boolean, };

export type RepositorySourceSummary = { id: string, name: string, abbreviation: string, repositories: Array<RepositorySummary>, };

export type ListRepositoriesResponse = { sources: Array<RepositorySourceSummary>, };

export type ChangeRequestState = "open" | "closed" | "merged";

export type ChangeRequestReviewStatus = "approved" | "changesRequested" | "reviewRequired" | "unknown";

export type ChangeRequestCheckStatus = "passed" | "failing" | "running" | "unknown";

export type ChangeRequestMergeStatus = "ready" | "blocked" | "conflicting" | "unknown";

export type ChangeRequestSummary = { id: string, number: number, title: string, author: string | null, sourceBranch: string, targetBranch: string, state: ChangeRequestState, draft: boolean, reviewStatus: ChangeRequestReviewStatus, checkStatus: ChangeRequestCheckStatus, mergeStatus: ChangeRequestMergeStatus, createdAt: string, updatedAt: string, webUrl: string, sourceId: string, sourceName: string, sourceAbbreviation: string, repositoryId: string, repositoryOwner: string, repositoryName: string, };

export type ListChangeRequestsResponse = { selectedRepositoryCount: number, changeRequests: Array<ChangeRequestSummary>, };

export type ChangeRequestActivityKind = "opened" | "readyForReview" | "reviewApproved" | "changesRequested" | "checksFailed" | "checksRecovered" | "conflictDetected" | "conflictResolved" | "merged" | "closed";

export type ChangeRequestActivitySummary = { id: string, kind: ChangeRequestActivityKind, occurredAt: string, changeRequest: ChangeRequestSummary, };

export type ListActivityResponse = { changeRequestEvents: Array<ChangeRequestActivitySummary>, };

export type ChangeRequestDetailsRequest = { sourceId: string, repositoryId: string, number: number, };

export type ChangeRequestReviewSummary = { reviewer: string | null, status: ChangeRequestReviewStatus, submittedAt: string | null, };

export type ChangeRequestCheckSummary = { name: string, status: ChangeRequestCheckStatus, webUrl: string | null, workflowRunId: string | null, };

export type ChangeRequestCommitSummary = { sha: string, title: string, author: string | null, committedAt: string, };

export type ChangeRequestDetailsResponse = { body: string | null, labels: Array<string>, reviews: Array<ChangeRequestReviewSummary>, checks: Array<ChangeRequestCheckSummary>, latestCommit: ChangeRequestCommitSummary | null, };

export type ChangeRequestDetailsErrorResponse = { message: string, };

export type IssueState = "open" | "closed";

export type IssueSummary = { id: string, number: number, title: string, author: string | null, state: IssueState, labels: Array<string>, assignees: Array<string>, commentCount: number, createdAt: string, updatedAt: string, webUrl: string, sourceId: string, sourceName: string, sourceAbbreviation: string, repositoryId: string, repositoryOwner: string, repositoryName: string, };

export type ListIssuesResponse = { selectedRepositoryCount: number, issues: Array<IssueSummary>, };

export type IssueDetailsRequest = { sourceId: string, repositoryId: string, number: number, };

export type IssueCommentSummary = { id: string, author: string | null, body: string, createdAt: string, updatedAt: string, };

export type IssueDetailsResponse = { title: string, state: IssueState, labels: Array<string>, assignees: Array<string>, commentCount: number, updatedAt: string, body: string | null, milestone: string | null, comments: Array<IssueCommentSummary>, };

export type IssueDetailsErrorResponse = { message: string, };

export type WorkflowState = "active" | "disabled";

export type RunLifecycle = "queued" | "running" | "completed" | "unknown";

export type RunOutcome = "success" | "warning" | "failure" | "cancelled" | "skipped" | "unknown";

export type WorkflowRunSummary = { id: string, runNumber: number, attempt: number, title: string, lifecycle: RunLifecycle, outcome: RunOutcome, branch: string | null, commitSha: string, actor: string | null, trigger: string, createdAt: string, startedAt: string | null, updatedAt: string, webUrl: string, };

export type WorkflowRunLogsRequest = { sourceId: string, repositoryId: string, runId: string, attempt: number | null, };

export type WorkflowRunLogFile = { name: string, content: string, };

export type WorkflowRunLogsResponse = { run: WorkflowRunSummary, files: Array<WorkflowRunLogFile>, truncated: boolean, };

export type WorkflowRunLogsErrorResponse = { message: string, };

export type WorkflowSummary = { id: string, name: string, path: string, state: WorkflowState, webUrl: string, sourceId: string, sourceName: string, sourceAbbreviation: string, repositoryId: string, repositoryOwner: string, repositoryName: string, runs: Array<WorkflowRunSummary>, };

export type ListWorkflowsResponse = { selectedRepositoryCount: number, workflows: Array<WorkflowSummary>, lastAttemptedAt: number | null, lastSuccessfulAt: number | null, stale: boolean, syncError: string | null, };

export type SynchronizationResponse = { selectedRepositoryCount: number, synchronizedRepositoryCount: number, failedRepositoryCount: number, skippedRepositoryCount: number, alreadyRunning: boolean, };

export type SynchronizationStatusResponse = { running: boolean, activeSourceCount: number, lastCompletedAt: number | null, lastFailedRepositoryCount: number, };

export type MonitoringSettingsResponse = { onlyMyWork: boolean, synchronizationIntervalSeconds: number, recentRunsPerWorkflow: number, };

export type UpdateMonitoringSettingsRequest = { onlyMyWork: boolean, synchronizationIntervalSeconds: number, recentRunsPerWorkflow: number, };

export type MonitoringSettingsErrorResponse = { message: string, };

export type RepositorySelectionSourceRequest = { sourceId: string, repositoryIds: Array<string>, };

export type SaveRepositorySelectionRequest = { sources: Array<RepositorySelectionSourceRequest>, };

export type SaveRepositorySelectionResponse = { selectedCount: number, };

export type RepositorySelectionErrorResponse = { message: string, };

export const applicationVersion = "0.4.0" as const;

export const settingsLimits = {
  synchronizationIntervalSeconds: { min: 30, max: 3600 },
  recentRunsPerWorkflow: { min: 1, max: 100 },
} as const;

export const httpRoutes = {
  health: "/api/health",
  updateStatus: "/api/update-status",
  workflows: "/api/workflows",
  activity: "/api/activity",
  changeRequests: "/api/change-requests",
  changeRequestDetails: "/api/change-request-details",
  issues: "/api/issues",
  issueDetails: "/api/issue-details",
  workflowRunLogs: "/api/workflow-run-logs",
  synchronization: "/api/sync",
  settings: "/api/settings",
  sources: "/api/sources",
  connections: "/api/connections",
  repositories: "/api/repositories",
  repositorySelections: "/api/repository-selections",
} as const;

export const desktopCommands = {
  health: "health",
  updateStatus: "update_status",
  listWorkflows: "list_workflows",
  listActivity: "list_activity",
  listChangeRequests: "list_change_requests",
  changeRequestDetails: "change_request_details",
  listIssues: "list_issues",
  issueDetails: "issue_details",
  workflowRunLogs: "workflow_run_logs",
  synchronizeSources: "synchronize_sources",
  synchronizationStatus: "synchronization_status",
  getSettings: "get_settings",
  updateSettings: "update_settings",
  listSources: "list_sources",
  connectSource: "connect_source",
  disconnectSource: "disconnect_source",
  listRepositories: "list_repositories",
  saveRepositorySelection: "save_repository_selection",
} as const;

export interface ApplicationClient {
  health(): Promise<HealthResponse>;
  updateStatus(): Promise<UpdateStatusResponse>;
  listWorkflows(): Promise<ListWorkflowsResponse>;
  listActivity(): Promise<ListActivityResponse>;
  listChangeRequests(): Promise<ListChangeRequestsResponse>;
  changeRequestDetails(request: ChangeRequestDetailsRequest): Promise<ChangeRequestDetailsResponse>;
  listIssues(): Promise<ListIssuesResponse>;
  issueDetails(request: IssueDetailsRequest): Promise<IssueDetailsResponse>;
  workflowRunLogs(request: WorkflowRunLogsRequest): Promise<WorkflowRunLogsResponse>;
  synchronizeSources(): Promise<SynchronizationResponse>;
  synchronizationStatus(): Promise<SynchronizationStatusResponse>;
  getSettings(): Promise<MonitoringSettingsResponse>;
  updateSettings(request: UpdateMonitoringSettingsRequest): Promise<MonitoringSettingsResponse>;
  listSources(): Promise<ListSourcesResponse>;
  connectSource(request: ConnectSourceRequest): Promise<ConnectionSummary>;
  disconnectSource(request: DisconnectSourceRequest): Promise<DisconnectSourceResponse>;
  listRepositories(): Promise<ListRepositoriesResponse>;
  saveRepositorySelection(request: SaveRepositorySelectionRequest): Promise<SaveRepositorySelectionResponse>;
}
