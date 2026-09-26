// Generated from crates/ciwatcher-core/src/contracts.rs. Do not edit.

export type HealthResponse = { status: string, service: string, contractVersion: number, };

export type ConnectSourceRequest = { sourceId: string, credential: string, };

export type ConnectionSummary = { externalId: string, name: string, handle: string | null, profileUrl: string | null, credentialStored: boolean, };

export type CredentialFieldSummary = { label: string, placeholder: string, help: string, };

export type SourceSummary = { id: string, name: string, description: string, abbreviation: string, credential: CredentialFieldSummary, connection: ConnectionSummary | null, };

export type ListSourcesResponse = { sources: Array<SourceSummary>, };

export type DisconnectSourceRequest = { sourceId: string, };

export type DisconnectSourceResponse = { disconnected: boolean, };

export type ConnectionValidationErrorCode = "invalidCredentials" | "permissionDenied" | "rateLimited" | "providerUnavailable" | "unexpectedResponse" | "storageUnavailable" | "unknownSource";

export type ConnectionValidationErrorResponse = { code: ConnectionValidationErrorCode, message: string, };

export type RepositoryVisibility = "public" | "private";

export type RepositorySummary = { id: string, owner: string, name: string, description: string | null, visibility: RepositoryVisibility, webUrl: string, selected: boolean, };

export type RepositorySourceSummary = { id: string, name: string, abbreviation: string, repositories: Array<RepositorySummary>, };

export type ListRepositoriesResponse = { sources: Array<RepositorySourceSummary>, };

export type WorkflowState = "active" | "disabled";

export type RunLifecycle = "queued" | "running" | "completed" | "unknown";

export type RunOutcome = "success" | "warning" | "failure" | "cancelled" | "skipped" | "unknown";

export type WorkflowRunSummary = { id: string, runNumber: number, attempt: number, title: string, lifecycle: RunLifecycle, outcome: RunOutcome, branch: string | null, commitSha: string, actor: string | null, trigger: string, createdAt: string, startedAt: string | null, updatedAt: string, webUrl: string, };

export type WorkflowSummary = { id: string, name: string, path: string, state: WorkflowState, webUrl: string, sourceId: string, sourceName: string, sourceAbbreviation: string, repositoryId: string, repositoryOwner: string, repositoryName: string, runs: Array<WorkflowRunSummary>, };

export type ListWorkflowsResponse = { selectedRepositoryCount: number, workflows: Array<WorkflowSummary>, lastAttemptedAt: number | null, lastSuccessfulAt: number | null, stale: boolean, syncError: string | null, };

export type RepositorySelectionSourceRequest = { sourceId: string, repositoryIds: Array<string>, };

export type SaveRepositorySelectionRequest = { sources: Array<RepositorySelectionSourceRequest>, };

export type SaveRepositorySelectionResponse = { selectedCount: number, };

export type RepositorySelectionErrorResponse = { message: string, };

export const httpRoutes = {
  health: "/api/health",
  workflows: "/api/workflows",
  sources: "/api/sources",
  connections: "/api/connections",
  repositories: "/api/repositories",
  repositorySelections: "/api/repository-selections",
} as const;

export const desktopCommands = {
  health: "health",
  listWorkflows: "list_workflows",
  listSources: "list_sources",
  connectSource: "connect_source",
  disconnectSource: "disconnect_source",
  listRepositories: "list_repositories",
  saveRepositorySelection: "save_repository_selection",
} as const;

export interface ApplicationClient {
  health(): Promise<HealthResponse>;
  listWorkflows(): Promise<ListWorkflowsResponse>;
  listSources(): Promise<ListSourcesResponse>;
  connectSource(request: ConnectSourceRequest): Promise<ConnectionSummary>;
  disconnectSource(request: DisconnectSourceRequest): Promise<DisconnectSourceResponse>;
  listRepositories(): Promise<ListRepositoriesResponse>;
  saveRepositorySelection(request: SaveRepositorySelectionRequest): Promise<SaveRepositorySelectionResponse>;
}
