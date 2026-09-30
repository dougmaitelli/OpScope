import { invoke } from "@tauri-apps/api/core";
import {
  desktopCommands,
  httpRoutes,
  type ApplicationClient,
  type ActionOptions,
  type ActionOptionsRequest,
  type ExecuteActionRequest,
  type ExecuteActionResponse,
  type ChangeRequestDetailsRequest,
  type ChangeRequestDetailsResponse,
  type ConnectionSummary,
  type ConnectionValidationErrorResponse,
  type ConnectSourceRequest,
  type DisconnectSourceRequest,
  type DisconnectSourceResponse,
  type HealthResponse,
  type IssueDetailsRequest,
  type IssueDetailsResponse,
  type ListActivityResponse,
  type ListWorkflowsResponse,
  type ListChangeRequestsResponse,
  type ListIssuesResponse,
  type ListRepositoriesResponse,
  type ListSourcesResponse,
  type MonitoringSettingsResponse,
  type SaveRepositorySelectionRequest,
  type SaveRepositorySelectionResponse,
  type SynchronizationResponse,
  type SynchronizationRequest,
  type SynchronizationStatusResponse,
  type UpdateMonitoringSettingsRequest,
  type UpdateStatusResponse,
  type WorkflowRunLogsRequest,
  type WorkflowRunLogsResponse,
} from "./generated/contracts.ts";

export class DesktopClient implements ApplicationClient {
  actionOptions(request: ActionOptionsRequest): Promise<ActionOptions> {
    return invoke<ActionOptions>(desktopCommands.actionOptions, { request });
  }

  executeAction(request: ExecuteActionRequest): Promise<ExecuteActionResponse> {
    return invoke<ExecuteActionResponse>(desktopCommands.executeAction, { request });
  }
  health(): Promise<HealthResponse> {
    return invoke<HealthResponse>(desktopCommands.health);
  }

  updateStatus(): Promise<UpdateStatusResponse> {
    return invoke<UpdateStatusResponse>(desktopCommands.updateStatus);
  }

  listWorkflows(): Promise<ListWorkflowsResponse> {
    return invoke<ListWorkflowsResponse>(desktopCommands.listWorkflows);
  }

  listActivity(): Promise<ListActivityResponse> {
    return invoke<ListActivityResponse>(desktopCommands.listActivity);
  }

  listChangeRequests(): Promise<ListChangeRequestsResponse> {
    return invoke<ListChangeRequestsResponse>(desktopCommands.listChangeRequests);
  }

  changeRequestDetails(
    request: ChangeRequestDetailsRequest,
  ): Promise<ChangeRequestDetailsResponse> {
    return invoke<ChangeRequestDetailsResponse>(desktopCommands.changeRequestDetails, { request });
  }

  listIssues(): Promise<ListIssuesResponse> {
    return invoke<ListIssuesResponse>(desktopCommands.listIssues);
  }

  issueDetails(request: IssueDetailsRequest): Promise<IssueDetailsResponse> {
    return invoke<IssueDetailsResponse>(desktopCommands.issueDetails, { request });
  }

  workflowRunLogs(request: WorkflowRunLogsRequest): Promise<WorkflowRunLogsResponse> {
    return invoke<WorkflowRunLogsResponse>(desktopCommands.workflowRunLogs, { request });
  }

  synchronizeSources(request: SynchronizationRequest): Promise<SynchronizationResponse> {
    return invoke<SynchronizationResponse>(desktopCommands.synchronizeSources, { request });
  }

  synchronizationStatus(): Promise<SynchronizationStatusResponse> {
    return invoke<SynchronizationStatusResponse>(desktopCommands.synchronizationStatus);
  }

  getSettings(): Promise<MonitoringSettingsResponse> {
    return invoke<MonitoringSettingsResponse>(desktopCommands.getSettings);
  }

  updateSettings(request: UpdateMonitoringSettingsRequest): Promise<MonitoringSettingsResponse> {
    return invoke<MonitoringSettingsResponse>(desktopCommands.updateSettings, { request });
  }

  listSources(): Promise<ListSourcesResponse> {
    return invoke<ListSourcesResponse>(desktopCommands.listSources);
  }

  listRepositories(): Promise<ListRepositoriesResponse> {
    return invoke<ListRepositoriesResponse>(desktopCommands.listRepositories);
  }

  saveRepositorySelection(
    request: SaveRepositorySelectionRequest,
  ): Promise<SaveRepositorySelectionResponse> {
    return invoke<SaveRepositorySelectionResponse>(desktopCommands.saveRepositorySelection, {
      request,
    });
  }

  connectSource(request: ConnectSourceRequest): Promise<ConnectionSummary> {
    return invoke<ConnectionSummary>(desktopCommands.connectSource, { request });
  }

  disconnectSource(request: DisconnectSourceRequest): Promise<DisconnectSourceResponse> {
    return invoke<DisconnectSourceResponse>(desktopCommands.disconnectSource, { request });
  }
}

export class HttpClient implements ApplicationClient {
  actionOptions(request: ActionOptionsRequest): Promise<ActionOptions> {
    return this.post<ActionOptions>(httpRoutes.actionOptions, request);
  }

  executeAction(request: ExecuteActionRequest): Promise<ExecuteActionResponse> {
    return this.post<ExecuteActionResponse>(httpRoutes.executeAction, request);
  }
  private csrfToken: string | null = null;
  private readonly fetcher: typeof fetch;

  constructor(fetcher: typeof fetch = (input, init) => globalThis.fetch(input, init)) {
    this.fetcher = fetcher;
  }

  setCsrfToken(csrfToken: string | null): void {
    this.csrfToken = csrfToken;
  }

  health(): Promise<HealthResponse> {
    return this.get<HealthResponse>(httpRoutes.health);
  }

  updateStatus(): Promise<UpdateStatusResponse> {
    return this.get<UpdateStatusResponse>(httpRoutes.updateStatus);
  }

  listWorkflows(): Promise<ListWorkflowsResponse> {
    return this.get<ListWorkflowsResponse>(httpRoutes.workflows);
  }

  listActivity(): Promise<ListActivityResponse> {
    return this.get<ListActivityResponse>(httpRoutes.activity);
  }

  listChangeRequests(): Promise<ListChangeRequestsResponse> {
    return this.get<ListChangeRequestsResponse>(httpRoutes.changeRequests);
  }

  changeRequestDetails(
    request: ChangeRequestDetailsRequest,
  ): Promise<ChangeRequestDetailsResponse> {
    return this.post<ChangeRequestDetailsResponse>(httpRoutes.changeRequestDetails, request);
  }

  listIssues(): Promise<ListIssuesResponse> {
    return this.get<ListIssuesResponse>(httpRoutes.issues);
  }

  issueDetails(request: IssueDetailsRequest): Promise<IssueDetailsResponse> {
    return this.post<IssueDetailsResponse>(httpRoutes.issueDetails, request);
  }

  workflowRunLogs(request: WorkflowRunLogsRequest): Promise<WorkflowRunLogsResponse> {
    return this.post<WorkflowRunLogsResponse>(httpRoutes.workflowRunLogs, request);
  }

  synchronizeSources(request: SynchronizationRequest): Promise<SynchronizationResponse> {
    return this.post<SynchronizationResponse>(httpRoutes.synchronization, request);
  }

  synchronizationStatus(): Promise<SynchronizationStatusResponse> {
    return this.get<SynchronizationStatusResponse>(httpRoutes.synchronization);
  }

  getSettings(): Promise<MonitoringSettingsResponse> {
    return this.get<MonitoringSettingsResponse>(httpRoutes.settings);
  }

  updateSettings(request: UpdateMonitoringSettingsRequest): Promise<MonitoringSettingsResponse> {
    return this.put<MonitoringSettingsResponse>(httpRoutes.settings, request);
  }

  listSources(): Promise<ListSourcesResponse> {
    return this.get<ListSourcesResponse>(httpRoutes.sources);
  }

  listRepositories(): Promise<ListRepositoriesResponse> {
    return this.get<ListRepositoriesResponse>(httpRoutes.repositories);
  }

  saveRepositorySelection(
    request: SaveRepositorySelectionRequest,
  ): Promise<SaveRepositorySelectionResponse> {
    return this.put<SaveRepositorySelectionResponse>(httpRoutes.repositorySelections, request);
  }

  connectSource(request: ConnectSourceRequest): Promise<ConnectionSummary> {
    return this.post<ConnectionSummary>(httpRoutes.connections, request);
  }

  disconnectSource(request: DisconnectSourceRequest): Promise<DisconnectSourceResponse> {
    return this.delete<DisconnectSourceResponse>(httpRoutes.connections, request);
  }

  private async get<T>(path: string): Promise<T> {
    const response = await this.fetcher(path, {
      headers: { Accept: "application/json" },
      credentials: "same-origin",
    });
    if (!response.ok) {
      throw await this.responseError(response);
    }
    return this.readJson<T>(response);
  }

  private async post<T>(path: string, body: unknown): Promise<T> {
    const response = await this.fetcher(path, {
      method: "POST",
      headers: {
        Accept: "application/json",
        "Content-Type": "application/json",
        ...this.csrfHeaders(),
      },
      credentials: "same-origin",
      body: JSON.stringify(body),
    });
    if (!response.ok) {
      throw await this.responseError(response);
    }
    return this.readJson<T>(response);
  }

  private async delete<T>(path: string, body: unknown): Promise<T> {
    const response = await this.fetcher(path, {
      method: "DELETE",
      headers: {
        Accept: "application/json",
        "Content-Type": "application/json",
        ...this.csrfHeaders(),
      },
      credentials: "same-origin",
      body: JSON.stringify(body),
    });
    if (!response.ok) {
      throw await this.responseError(response);
    }
    return this.readJson<T>(response);
  }

  private async put<T>(path: string, body: unknown): Promise<T> {
    const response = await this.fetcher(path, {
      method: "PUT",
      headers: {
        Accept: "application/json",
        "Content-Type": "application/json",
        ...this.csrfHeaders(),
      },
      credentials: "same-origin",
      body: JSON.stringify(body),
    });
    if (!response.ok) {
      throw await this.responseError(response);
    }
    return this.readJson<T>(response);
  }

  private async responseError(response: Response): Promise<Error> {
    if (
      response.status === 401 &&
      response.headers.get("x-opsscope-session-error") === "unauthenticated"
    ) {
      globalThis.dispatchEvent(new Event("opsscope:unauthorized"));
    }
    const fallback =
      response.status === 404
        ? "The application endpoint is unavailable. Restart the application service."
        : `The application service returned status ${response.status}.`;
    const text = await response.text();
    if (text.length === 0) {
      return new Error(fallback);
    }

    try {
      const failure = JSON.parse(text) as Partial<ConnectionValidationErrorResponse>;
      return new Error(typeof failure.message === "string" ? failure.message : fallback);
    } catch {
      return new Error(fallback);
    }
  }

  private csrfHeaders(): Record<string, string> {
    return this.csrfToken ? { "X-CSRF-Token": this.csrfToken } : {};
  }

  private async readJson<T>(response: Response): Promise<T> {
    const text = await response.text();
    if (text.length === 0) {
      throw new Error("The application service returned an empty response.");
    }

    try {
      return JSON.parse(text) as T;
    } catch {
      throw new Error("The application service returned an invalid response.");
    }
  }
}
