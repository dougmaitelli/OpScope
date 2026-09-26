import { DesktopClient, HttpClient } from "./clients.ts";
import type {
  ApplicationClient,
  ConnectionSummary,
  ListMonitorsResponse,
  MonitorStatus,
  MonitorSummary,
  SourceSummary,
} from "./generated/contracts.ts";
import "./styles.css";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

function requiredElement<T extends Element>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (!element) {
    throw new Error(`application shell is missing ${selector}`);
  }
  return element;
}

const client: ApplicationClient = window.__TAURI_INTERNALS__
  ? new DesktopClient()
  : new HttpClient();

const summary = requiredElement<HTMLElement>(".health-summary");
const healthIcon = requiredElement<HTMLSpanElement>("#health-icon");
const healthHeading = requiredElement<HTMLHeadingElement>("#health-heading");
const lastUpdated = requiredElement<HTMLParagraphElement>("#last-updated");
const failingCount = requiredElement<HTMLElement>("#failing-count");
const runningCount = requiredElement<HTMLElement>("#running-count");
const passingCount = requiredElement<HTMLElement>("#passing-count");
const monitorList = requiredElement<HTMLDivElement>("#monitor-list");
const refreshButton = requiredElement<HTMLButtonElement>("#refresh-button");
const serviceDot = requiredElement<HTMLSpanElement>("#service-dot");
const serviceState = requiredElement<HTMLSpanElement>("#service-state");
const overviewNav = requiredElement<HTMLAnchorElement>("#overview-nav");
const connectionsNav = requiredElement<HTMLAnchorElement>("#connections-nav");
const overviewView = requiredElement<HTMLElement>("#overview-view");
const connectionsView = requiredElement<HTMLElement>("#connections-view");
const sourceList = requiredElement<HTMLDivElement>("#source-list");
const sourceCount = requiredElement<HTMLSpanElement>("#source-count");
const connectionEditor = requiredElement<HTMLElement>("#connection-editor");
const connectionForm = requiredElement<HTMLFormElement>("#connection-form");
const connectButton = requiredElement<HTMLButtonElement>("#connect-button");
const cancelConnection = requiredElement<HTMLButtonElement>("#cancel-connection");
const credentialInput = requiredElement<HTMLInputElement>("#source-credential");
const credentialLabel = requiredElement<HTMLLabelElement>("#credential-label");
const credentialHelp = requiredElement<HTMLParagraphElement>("#credential-help");
const connectionHeading = requiredElement<HTMLHeadingElement>("#connection-heading");
const connectionNote = requiredElement<HTMLParagraphElement>("#connection-note");

let sources: SourceSummary[] = [];
let activeSourceId: string | null = null;
let pendingDisconnectId: string | null = null;

const statusLabels: Record<MonitorStatus, string> = {
  unknown: "Unknown",
  passing: "Passing",
  failing: "Failing",
  running: "Running",
};

function countStatus(response: ListMonitorsResponse, status: MonitorStatus): number {
  return response.monitors.filter((monitor) => monitor.status === status).length;
}

function renderMonitor(monitor: MonitorSummary): HTMLElement {
  const row = document.createElement("article");
  row.className = "workflow-row";

  const identity = document.createElement("div");
  identity.className = "workflow-identity";

  const mark = document.createElement("span");
  mark.className = "workflow-mark";
  mark.setAttribute("aria-hidden", "true");
  mark.textContent = monitor.name.slice(0, 2).toUpperCase();

  const copy = document.createElement("div");
  const name = document.createElement("h3");
  name.textContent = monitor.name;
  const context = document.createElement("p");
  context.textContent = "Demo source · workflow monitor";
  copy.append(name, context);
  identity.append(mark, copy);

  const state = document.createElement("span");
  state.className = `monitor-status monitor-status-${monitor.status}`;
  state.textContent = statusLabels[monitor.status];

  row.append(identity, state);
  return row;
}

function renderMonitors(response: ListMonitorsResponse): void {
  monitorList.replaceChildren();

  if (response.monitors.length === 0) {
    const empty = document.createElement("p");
    empty.className = "empty-state";
    empty.textContent = "No workflows are being monitored yet.";
    monitorList.append(empty);
    return;
  }

  for (const monitor of response.monitors) {
    monitorList.append(renderMonitor(monitor));
  }
}

function renderSummary(response: ListMonitorsResponse): void {
  const failing = countStatus(response, "failing");
  const running = countStatus(response, "running");
  const passing = countStatus(response, "passing");

  failingCount.textContent = String(failing);
  runningCount.textContent = String(running);
  passingCount.textContent = String(passing);

  healthIcon.className = "health-icon";
  if (failing > 0) {
    healthIcon.classList.add("health-icon-failing");
    healthIcon.textContent = "!";
    healthHeading.textContent = `${failing} workflow${failing === 1 ? "" : "s"} need attention`;
  } else if (running > 0) {
    healthIcon.classList.add("health-icon-running");
    healthIcon.textContent = "↻";
    healthHeading.textContent = "Work is currently in progress";
  } else if (response.monitors.length > 0) {
    healthIcon.classList.add("health-icon-passing");
    healthIcon.textContent = "✓";
    healthHeading.textContent = "Everything looks healthy";
  } else {
    healthIcon.classList.add("health-icon-idle");
    healthIcon.textContent = "·";
    healthHeading.textContent = "Ready for your first connection";
  }
}

function setUnavailable(): void {
  serviceDot.classList.remove("service-dot-online");
  serviceDot.classList.add("service-dot-error");
  serviceState.textContent = "Core unavailable";
  healthIcon.className = "health-icon health-icon-failing";
  healthIcon.textContent = "!";
  healthHeading.textContent = "Unable to load workflow health";
  lastUpdated.textContent = "Check that the application service is running";
  failingCount.textContent = "—";
  runningCount.textContent = "—";
  passingCount.textContent = "—";
  monitorList.replaceChildren();
  const error = document.createElement("p");
  error.className = "empty-state empty-state-error";
  error.textContent = "The dashboard could not reach the application core.";
  monitorList.append(error);
}

async function loadDashboard(): Promise<void> {
  refreshButton.disabled = true;
  refreshButton.classList.add("button-busy");
  summary.setAttribute("aria-busy", "true");

  try {
    const [health, response] = await Promise.all([
      client.health(),
      client.listMonitors(),
    ]);
    serviceDot.classList.remove("service-dot-error");
    serviceDot.classList.add("service-dot-online");
    serviceState.textContent = `Core online · contract v${health.contractVersion}`;
    lastUpdated.textContent = "Updated just now · synthetic provider";
    renderSummary(response);
    renderMonitors(response);
  } catch {
    setUnavailable();
  } finally {
    refreshButton.disabled = false;
    refreshButton.classList.remove("button-busy");
    summary.setAttribute("aria-busy", "false");
  }
}

refreshButton.addEventListener("click", () => void loadDashboard());

type AppView = "overview" | "connections";

const viewPaths: Record<AppView, string> = {
  overview: "/",
  connections: "/connections",
};

function viewFromPath(pathname: string): AppView | null {
  const normalizedPath = pathname.replace(/\/+$/, "") || "/";
  if (normalizedPath === viewPaths.overview) {
    return "overview";
  }
  if (normalizedPath === viewPaths.connections) {
    return "connections";
  }
  return null;
}

function showView(view: AppView): void {
  const showOverview = view === "overview";
  overviewView.hidden = !showOverview;
  connectionsView.hidden = showOverview;
  overviewNav.classList.toggle("nav-item-active", showOverview);
  connectionsNav.classList.toggle("nav-item-active", !showOverview);
  if (showOverview) {
    overviewNav.setAttribute("aria-current", "page");
    connectionsNav.removeAttribute("aria-current");
  } else {
    overviewNav.removeAttribute("aria-current");
    connectionsNav.setAttribute("aria-current", "page");
  }
  document.title = `${showOverview ? "Overview" : "Connections"} · CI Watcher`;
}

function navigateTo(view: AppView): void {
  if (window.location.pathname !== viewPaths[view]) {
    window.history.pushState(null, "", viewPaths[view]);
  }
  showView(view);
}

function bindNavigation(link: HTMLAnchorElement, view: AppView): void {
  link.addEventListener("click", (event) => {
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
      return;
    }
    event.preventDefault();
    navigateTo(view);
  });
}

function showCurrentRoute(): void {
  const view = viewFromPath(window.location.pathname);
  if (view) {
    showView(view);
    return;
  }
  window.history.replaceState(null, "", viewPaths.overview);
  showView("overview");
}

bindNavigation(overviewNav, "overview");
bindNavigation(connectionsNav, "connections");
window.addEventListener("popstate", showCurrentRoute);
showCurrentRoute();

function openSourceEditor(source: SourceSummary): void {
  activeSourceId = source.id;
  pendingDisconnectId = null;
  connectionHeading.textContent = source.connection
    ? `Replace ${source.name} credential`
    : `Connect ${source.name}`;
  credentialLabel.textContent = source.credential.label;
  credentialInput.placeholder = source.credential.placeholder;
  credentialHelp.textContent = `${source.credential.help} The credential is never returned to this interface after submission.`;
  connectButton.textContent = source.connection ? "Replace credential" : `Connect ${source.name}`;
  connectionEditor.hidden = false;
  credentialInput.focus();
  renderSources();
}

function accountLabel(connection: ConnectionSummary): string {
  return connection.handle ? `${connection.name} · ${connection.handle}` : connection.name;
}

function sourceStatus(connected: boolean): HTMLElement {
  const status = document.createElement("div");
  status.className = "connection-status";
  const dot = document.createElement("span");
  dot.className = `status-dot ${connected ? "status-dot-connected" : "status-dot-muted"}`;
  dot.setAttribute("aria-hidden", "true");
  const label = document.createElement("span");
  label.textContent = connected ? "Connected" : "Not connected";
  status.append(dot, label);
  return status;
}

function renderSource(source: SourceSummary): HTMLElement {
  const wrapper = document.createElement("article");
  wrapper.className = "source-entry";

  const row = document.createElement("div");
  row.className = "source-row";
  const identity = document.createElement("div");
  identity.className = "source-identity";
  const mark = document.createElement("span");
  mark.className = "source-mark";
  mark.setAttribute("aria-hidden", "true");
  mark.textContent = source.abbreviation;
  const copy = document.createElement("div");
  const name = document.createElement("h3");
  name.textContent = source.name;
  const description = document.createElement("p");
  description.textContent = source.description;
  copy.append(name, description);
  identity.append(mark, copy);

  const state = document.createElement("div");
  state.className = "source-state";
  state.append(sourceStatus(source.connection !== null));
  const actions = document.createElement("div");
  actions.className = "source-actions";
  const configure = document.createElement("button");
  configure.className = "secondary-button compact-button";
  configure.type = "button";
  configure.textContent = source.connection ? "Replace credential" : "Connect";
  configure.addEventListener("click", () => openSourceEditor(source));
  actions.append(configure);
  if (source.connection) {
    const disconnect = document.createElement("button");
    disconnect.className = "danger-button compact-button";
    disconnect.type = "button";
    disconnect.textContent = "Disconnect";
    disconnect.addEventListener("click", () => {
      pendingDisconnectId = source.id;
      connectionEditor.hidden = true;
      renderSources();
    });
    actions.append(disconnect);
  }
  state.append(actions);
  row.append(identity, state);
  wrapper.append(row);

  if (source.connection) {
    const account = document.createElement("div");
    account.className = "connected-account";
    const accountMark = document.createElement("span");
    accountMark.className = "account-mark";
    accountMark.setAttribute("aria-hidden", "true");
    accountMark.textContent = source.abbreviation;
    const accountCopy = document.createElement("div");
    const accountName = document.createElement("strong");
    accountName.textContent = accountLabel(source.connection);
    const accountId = document.createElement("span");
    accountId.textContent = "Connected account";
    accountCopy.append(accountName, accountId);
    const secure = document.createElement("span");
    secure.className = "secure-label";
    secure.textContent = "Credential stored securely";
    account.append(accountMark, accountCopy, secure);
    wrapper.append(account);
  }

  if (pendingDisconnectId === source.id) {
    const confirmation = document.createElement("div");
    confirmation.className = "disconnect-confirmation";
    const prompt = document.createElement("p");
    prompt.textContent = `Remove the ${source.name} connection and its stored credential?`;
    const confirmationActions = document.createElement("div");
    const cancel = document.createElement("button");
    cancel.className = "secondary-button compact-button";
    cancel.type = "button";
    cancel.textContent = "Cancel";
    cancel.addEventListener("click", () => {
      pendingDisconnectId = null;
      renderSources();
    });
    const confirm = document.createElement("button");
    confirm.className = "danger-button compact-button";
    confirm.type = "button";
    confirm.textContent = "Remove connection";
    confirm.addEventListener("click", () => void disconnectSource(source, confirm));
    confirmationActions.append(cancel, confirm);
    confirmation.append(prompt, confirmationActions);
    wrapper.append(confirmation);
  }

  return wrapper;
}

function renderSources(): void {
  sourceList.replaceChildren();
  const connected = sources.filter((source) => source.connection !== null).length;
  sourceCount.textContent = `${connected} connected`;

  if (sources.length === 0) {
    const empty = document.createElement("p");
    empty.className = "empty-state";
    empty.textContent = "No source modules are registered.";
    sourceList.append(empty);
    return;
  }

  for (const source of sources) {
    sourceList.append(renderSource(source));
  }
}

async function loadSources(): Promise<void> {
  try {
    const response = await client.listSources();
    sources = response.sources;
    renderSources();
  } catch (error: unknown) {
    sourceCount.textContent = "Unavailable";
    sourceList.replaceChildren();
    const message = document.createElement("p");
    message.className = "empty-state empty-state-error";
    message.textContent = connectionErrorMessage(error);
    sourceList.append(message);
  }
}

function connectionErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "object" && error !== null && "message" in error) {
    const message = (error as { message: unknown }).message;
    if (typeof message === "string") {
      return message;
    }
  }
  return "The connection could not be validated. Try again.";
}

connectionForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  const source = sources.find((candidate) => candidate.id === activeSourceId);
  let credential = credentialInput.value.trim();
  if (!source || credential.length === 0) {
    credentialInput.focus();
    return;
  }

  connectButton.disabled = true;
  connectButton.textContent = "Connecting…";
  connectionForm.setAttribute("aria-busy", "true");
  connectionNote.classList.remove("connection-note-error");
  connectionNote.textContent = `Contacting ${source.name}…`;

  try {
    const connection = await client.connectSource({
      sourceId: source.id,
      credential,
    });
    source.connection = connection;
    connectionEditor.hidden = true;
    renderSources();
    connectionNote.textContent = `${source.name} connected. Repository selection is the next step.`;
  } catch (error: unknown) {
    connectionNote.classList.add("connection-note-error");
    connectionNote.textContent = connectionErrorMessage(error);
  } finally {
    credential = "";
    credentialInput.value = "";
    connectButton.disabled = false;
    connectButton.textContent = source.connection
      ? "Replace credential"
      : `Connect ${source.name}`;
    connectionForm.setAttribute("aria-busy", "false");
  }
});

cancelConnection.addEventListener("click", () => {
  activeSourceId = null;
  credentialInput.value = "";
  connectionEditor.hidden = true;
});

async function disconnectSource(
  source: SourceSummary,
  button: HTMLButtonElement,
): Promise<void> {
  button.disabled = true;
  button.textContent = "Removing…";
  connectionNote.classList.remove("connection-note-error");
  try {
    await client.disconnectSource({ sourceId: source.id });
    source.connection = null;
    pendingDisconnectId = null;
    renderSources();
    connectionNote.textContent = `${source.name} disconnected and its credential removed.`;
  } catch (error: unknown) {
    button.disabled = false;
    button.textContent = "Remove connection";
    connectionNote.classList.add("connection-note-error");
    connectionNote.textContent = connectionErrorMessage(error);
  }
}

await Promise.all([loadDashboard(), loadSources()]);
