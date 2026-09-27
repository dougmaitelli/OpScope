import { createContext, type ReactNode, useContext } from "react";
import { DesktopClient, HttpClient } from "../clients.ts";
import type { ApplicationClient } from "../generated/contracts.ts";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export const isDesktopRuntime = Boolean(window.__TAURI_INTERNALS__);
const httpClient = new HttpClient();
const client: ApplicationClient = isDesktopRuntime ? new DesktopClient() : httpClient;

export function setHttpCsrfToken(csrfToken: string | null): void {
  httpClient.setCsrfToken(csrfToken);
}

const ApplicationClientContext = createContext<ApplicationClient | null>(null);

export function ApplicationClientProvider({ children }: { children: ReactNode }) {
  return (
    <ApplicationClientContext.Provider value={client}>{children}</ApplicationClientContext.Provider>
  );
}

export function useApplicationClient(): ApplicationClient {
  const value = useContext(ApplicationClientContext);
  if (!value) {
    throw new Error("application client provider is missing");
  }
  return value;
}
