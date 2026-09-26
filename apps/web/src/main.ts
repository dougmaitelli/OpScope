import { DesktopClient, HttpClient } from "./clients.ts";
import type { ApplicationClient } from "./generated/contracts.ts";
import "./styles.css";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}
const connection = document.querySelector<HTMLParagraphElement>("#connection");
const monitorList = document.querySelector<HTMLUListElement>("#monitors");

if (!connection || !monitorList) {
  throw new Error("application shell is incomplete");
}

const client: ApplicationClient = window.__TAURI_INTERNALS__
  ? new DesktopClient()
  : new HttpClient();

try {
  const [health, response] = await Promise.all([
    client.health(),
    client.listMonitors(),
  ]);
  connection.textContent = `${health.service} contract v${health.contractVersion}`;

  for (const monitor of response.monitors) {
    const item = document.createElement("li");
    item.textContent = `${monitor.name}: ${monitor.status}`;
    monitorList.append(item);
  }
} catch {
  connection.textContent = "Application service unavailable";
}
