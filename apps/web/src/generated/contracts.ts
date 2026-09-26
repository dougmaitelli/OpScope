// Generated from crates/ciwatcher-core/src/contracts.rs. Do not edit.

export type HealthResponse = { status: string, service: string, contractVersion: number, };

export type MonitorStatus = "unknown" | "passing" | "failing" | "running";

export type MonitorSummary = { id: string, name: string, status: MonitorStatus, };

export type ListMonitorsResponse = { monitors: Array<MonitorSummary>, };

export const httpRoutes = {
  health: "/api/health",
  listMonitors: "/api/monitors",
} as const;

export const desktopCommands = {
  health: "health",
  listMonitors: "list_monitors",
} as const;

export interface ApplicationClient {
  health(): Promise<HealthResponse>;
  listMonitors(): Promise<ListMonitorsResponse>;
}
