// Generated from crates/ciwatcher-core/src/contracts.rs. Do not edit.

export type HealthResponse = { status: string, service: string, contractVersion: number, };

export type MonitorStatus = "unknown" | "passing" | "failing" | "running";

export type MonitorSummary = { id: string, name: string, status: MonitorStatus, };

export type ListMonitorsResponse = { monitors: Array<MonitorSummary>, };

export type ConnectSourceRequest = { sourceId: string, credential: string, };

export type ConnectionSummary = { externalId: string, name: string, handle: string | null, profileUrl: string | null, credentialStored: boolean, };

export type CredentialFieldSummary = { label: string, placeholder: string, help: string, };

export type SourceSummary = { id: string, name: string, description: string, abbreviation: string, credential: CredentialFieldSummary, connection: ConnectionSummary | null, };

export type ListSourcesResponse = { sources: Array<SourceSummary>, };

export type DisconnectSourceRequest = { sourceId: string, };

export type DisconnectSourceResponse = { disconnected: boolean, };

export type ConnectionValidationErrorCode = "invalidCredentials" | "rateLimited" | "providerUnavailable" | "unexpectedResponse" | "storageUnavailable" | "unknownSource";

export type ConnectionValidationErrorResponse = { code: ConnectionValidationErrorCode, message: string, };

export const httpRoutes = {
  health: "/api/health",
  listMonitors: "/api/monitors",
  sources: "/api/sources",
  connections: "/api/connections",
} as const;

export const desktopCommands = {
  health: "health",
  listMonitors: "list_monitors",
  listSources: "list_sources",
  connectSource: "connect_source",
  disconnectSource: "disconnect_source",
} as const;

export interface ApplicationClient {
  health(): Promise<HealthResponse>;
  listMonitors(): Promise<ListMonitorsResponse>;
  listSources(): Promise<ListSourcesResponse>;
  connectSource(request: ConnectSourceRequest): Promise<ConnectionSummary>;
  disconnectSource(request: DisconnectSourceRequest): Promise<DisconnectSourceResponse>;
}
