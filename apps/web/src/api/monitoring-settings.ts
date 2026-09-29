import { createContext, useContext } from "react";
import type { MonitoringSettingsResponse } from "../generated/contracts.ts";

export const MonitoringSettingsContext = createContext<{
  settings: MonitoringSettingsResponse | null;
  error: string | null;
  publish(settings: MonitoringSettingsResponse): void;
  reload(): Promise<void>;
} | null>(null);

export function useMonitoringSettings() {
  const context = useContext(MonitoringSettingsContext);
  if (!context) throw new Error("monitoring settings provider is missing");
  return context;
}
