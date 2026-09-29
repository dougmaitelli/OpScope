import { useCallback, useEffect, useState, type ReactNode } from "react";
import { useApplicationClient } from "./application-client.tsx";
import { MonitoringSettingsContext } from "./monitoring-settings.ts";
import type { MonitoringSettingsResponse } from "../generated/contracts.ts";
import { requestErrorMessage } from "../shared/errors.ts";

export function MonitoringSettingsProvider({ children }: { children: ReactNode }) {
  const client = useApplicationClient();
  const [settings, setSettings] = useState<MonitoringSettingsResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const reload = useCallback(async () => {
    try {
      setSettings(await client.getSettings());
      setError(null);
    } catch (failure) {
      setError(requestErrorMessage(failure));
    }
  }, [client]);
  useEffect(() => {
    void reload();
  }, [reload]);
  return (
    <MonitoringSettingsContext.Provider value={{ settings, error, publish: setSettings, reload }}>
      {children}
    </MonitoringSettingsContext.Provider>
  );
}
