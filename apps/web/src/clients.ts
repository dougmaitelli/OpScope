import { invoke } from "@tauri-apps/api/core";
import {
  desktopCommands,
  httpRoutes,
  type ApplicationClient,
  type HealthResponse,
  type ListMonitorsResponse,
} from "./generated/contracts.ts";

export class DesktopClient implements ApplicationClient {
  health(): Promise<HealthResponse> {
    return invoke<HealthResponse>(desktopCommands.health);
  }

  listMonitors(): Promise<ListMonitorsResponse> {
    return invoke<ListMonitorsResponse>(desktopCommands.listMonitors);
  }
}
export class HttpClient implements ApplicationClient {
  constructor(private readonly fetcher: typeof fetch = fetch) {}

  health(): Promise<HealthResponse> {
    return this.get<HealthResponse>(httpRoutes.health);
  }

  listMonitors(): Promise<ListMonitorsResponse> {
    return this.get<ListMonitorsResponse>(httpRoutes.listMonitors);
  }

  private async get<T>(path: string): Promise<T> {
    const response = await this.fetcher(path, {
      headers: { Accept: "application/json" },
      credentials: "same-origin",
    });
    if (!response.ok) {
      throw new Error(`request failed with status ${response.status}`);
    }
    return response.json() as Promise<T>;
  }
}
