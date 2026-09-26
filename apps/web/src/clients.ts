import { invoke } from "@tauri-apps/api/core";
import {
  desktopCommands,
  httpRoutes,
  type ApplicationClient,
  type ConnectionSummary,
  type ConnectionValidationErrorResponse,
  type ConnectSourceRequest,
  type DisconnectSourceRequest,
  type DisconnectSourceResponse,
  type HealthResponse,
  type ListWorkflowsResponse,
  type ListRepositoriesResponse,
  type ListSourcesResponse,
  type SaveRepositorySelectionRequest,
  type SaveRepositorySelectionResponse,
  type SynchronizationResponse,
  type SynchronizationStatusResponse,
} from "./generated/contracts.ts";

export class DesktopClient implements ApplicationClient {
  health(): Promise<HealthResponse> {
    return invoke<HealthResponse>(desktopCommands.health);
  }

  listWorkflows(): Promise<ListWorkflowsResponse> {
    return invoke<ListWorkflowsResponse>(desktopCommands.listWorkflows);
  }

  synchronizeSources(): Promise<SynchronizationResponse> {
    return invoke<SynchronizationResponse>(desktopCommands.synchronizeSources);
  }

  synchronizationStatus(): Promise<SynchronizationStatusResponse> {
    return invoke<SynchronizationStatusResponse>(desktopCommands.synchronizationStatus);
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
    return invoke<SaveRepositorySelectionResponse>(
      desktopCommands.saveRepositorySelection,
      { request },
    );
  }

  connectSource(request: ConnectSourceRequest): Promise<ConnectionSummary> {
    return invoke<ConnectionSummary>(desktopCommands.connectSource, { request });
  }

  disconnectSource(request: DisconnectSourceRequest): Promise<DisconnectSourceResponse> {
    return invoke<DisconnectSourceResponse>(desktopCommands.disconnectSource, { request });
  }
}

export class HttpClient implements ApplicationClient {
  constructor(
    private readonly fetcher: typeof fetch = (input, init) =>
      globalThis.fetch(input, init),
  ) {}

  health(): Promise<HealthResponse> {
    return this.get<HealthResponse>(httpRoutes.health);
  }

  listWorkflows(): Promise<ListWorkflowsResponse> {
    return this.get<ListWorkflowsResponse>(httpRoutes.workflows);
  }

  synchronizeSources(): Promise<SynchronizationResponse> {
    return this.post<SynchronizationResponse>(httpRoutes.synchronization, {});
  }

  synchronizationStatus(): Promise<SynchronizationStatusResponse> {
    return this.get<SynchronizationStatusResponse>(httpRoutes.synchronization);
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
    return this.put<SaveRepositorySelectionResponse>(
      httpRoutes.repositorySelections,
      request,
    );
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
