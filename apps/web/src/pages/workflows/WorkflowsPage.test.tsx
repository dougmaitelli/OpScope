import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, useLocation } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ApplicationClientProvider } from "../../api/application-client.tsx";
import type { ListWorkflowsResponse, WorkflowSummary } from "../../generated/contracts.ts";
import { WorkflowsPage } from "./WorkflowsPage.tsx";

function workflow(
  repositoryName: string,
  lifecycle: "running" | "completed",
  outcome: "failure" | "success" | "unknown",
): WorkflowSummary {
  return {
    id: repositoryName,
    name: `${repositoryName} build`,
    path: ".github/workflows/build.yml",
    state: "active",
    webUrl: "https://github.com/team/app/actions",
    sourceId: "github",
    sourceName: "GitHub",
    sourceAbbreviation: "GH",
    repositoryId: repositoryName,
    repositoryOwner: "team",
    repositoryName,
    runs: [
      {
        id: `${repositoryName}-run`,
        runNumber: 1,
        attempt: 1,
        title: `${repositoryName} changes`,
        lifecycle,
        outcome,
        branch: "main",
        commitSha: "abcdef123456",
        actor: "developer",
        trigger: "push",
        createdAt: "2026-09-28T10:00:00Z",
        startedAt: "2026-09-28T10:00:00Z",
        updatedAt: "2026-09-28T10:01:00Z",
        webUrl: "https://github.com/team/app/actions/runs/1",
      },
    ],
  };
}

const inventory: ListWorkflowsResponse = {
  selectedRepositoryCount: 3,
  workflows: [
    workflow("broken", "completed", "failure"),
    workflow("busy", "running", "unknown"),
    workflow("healthy", "completed", "success"),
  ],
  lastAttemptedAt: 1,
  lastSuccessfulAt: 1,
  stale: false,
  syncError: null,
};

function CurrentLocation() {
  const location = useLocation();
  return (
    <output aria-label="Current location">
      {location.pathname}
      {location.search}
    </output>
  );
}

function renderPage(route = "/workflows") {
  return render(
    <ApplicationClientProvider>
      <MemoryRouter initialEntries={[route]}>
        <WorkflowsPage />
        <CurrentLocation />
      </MemoryRouter>
    </ApplicationClientProvider>,
  );
}

beforeEach(() => {
  vi.stubGlobal(
    "fetch",
    vi.fn<typeof fetch>(async (input) => {
      const url = String(input);
      if (url === "/api/workflows") return Response.json(inventory);
      if (url === "/api/sync")
        return Response.json({
          running: false,
          activeSourceCount: 0,
          lastCompletedAt: null,
          lastFailedRepositoryCount: 0,
        });
      throw new Error(`Unexpected request: ${url}`);
    }),
  );
});

describe("WorkflowsPage", () => {
  it("requests a workflows-only refresh", async () => {
    const fetchMock = vi.mocked(fetch);
    const fallback = fetchMock.getMockImplementation()!;
    fetchMock.mockImplementation(async (input, init) => {
      if (String(input) === "/api/sync" && init?.method === "POST") {
        expect(JSON.parse(String(init.body))).toEqual({ scope: "workflows" });
        return Response.json({ failedRepositoryCount: 0, alreadyRunning: false });
      }
      return fallback(input, init);
    });
    const user = userEvent.setup();
    renderPage();
    await screen.findByRole("heading", { name: "team/broken" });
    await user.click(screen.getByRole("button", { name: "Refresh" }));
    expect(
      fetchMock.mock.calls.filter(([url, init]) => url === "/api/sync" && init?.method === "POST"),
    ).toHaveLength(1);
  });
  it("updates the rerun dialog and only its workflow row without reloading the inventory", async () => {
    const original = inventory.workflows[0]!.runs[0]!;
    const updated = { ...original, attempt: 2, lifecycle: "queued", outcome: "unknown" };
    let accepted = false;
    const fetchMock = vi.mocked(fetch);
    const fallback = fetchMock.getMockImplementation()!;
    fetchMock.mockImplementation(async (input, init) => {
      const url = String(input);
      if (url === "/api/action-options")
        return Response.json({
          actions: [
            {
              action: "rerunWorkflow",
              label: "Re-run workflow",
              confirmation: "Rerun?",
              disabledReason: null,
            },
          ],
          revision: "1",
        });
      if (url === "/api/actions") {
        accepted = true;
        return Response.json({ accepted: true, run: updated, changeRequest: null, details: null });
      }
      if (url.startsWith("/api/workflow-run-logs"))
        return Response.json({
          run: accepted ? updated : original,
          files: [],
          truncated: false,
        });
      return fallback(input, init);
    });
    const user = userEvent.setup();
    renderPage();
    await user.click(await screen.findByRole("button", { name: "#1 · broken changes" }));
    const dialog = screen.getByRole("dialog");
    await user.click(await within(dialog).findByRole("button", { name: "Re-run workflow" }));
    await user.click(within(dialog).getByRole("button", { name: "Confirm action" }));
    expect(await within(dialog).findByText(/attempt 2/)).toBeInTheDocument();
    await waitFor(() => expect(within(dialog).getByText("Queued")).toBeInTheDocument());
    await user.click(within(dialog).getByRole("button", { name: "Close logs" }));
    expect(screen.getByText("Queued")).toBeInTheDocument();
    expect(screen.getByText("healthy build")).toBeInTheDocument();
    expect(fetchMock.mock.calls.filter(([url]) => url === "/api/workflows")).toHaveLength(1);
    expect(fetchMock.mock.calls.filter(([url]) => url === "/api/actions")).toHaveLength(1);
  });

  it("groups repositories and initially expands only failures", async () => {
    renderPage();
    expect(await screen.findByRole("heading", { name: "team/broken" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "team/busy" })).toBeInTheDocument();
    expect(screen.getByText("broken build")).toBeVisible();
    expect(screen.getByText("busy build")).not.toBeVisible();
    expect(screen.getByText("healthy build")).not.toBeVisible();
  });

  it.each([
    ["failing", "broken"],
    ["running", "busy"],
  ])("filters via the %s total and persists the filter in the URL", async (status, repository) => {
    const user = userEvent.setup();
    renderPage();
    await user.click(await screen.findByRole("button", { name: `Show 1 ${status} workflow` }));
    expect(screen.getByRole("combobox", { name: "Status" })).toHaveValue(status);
    expect(screen.getByLabelText("Current location")).toHaveTextContent(
      `/workflows?status=${status}`,
    );
    expect(screen.getByRole("heading", { name: `team/${repository}` })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "team/healthy" })).not.toBeInTheDocument();
    expect(
      screen.queryByRole("heading", {
        name: repository === "broken" ? "team/busy" : "team/broken",
      }),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Clear" }));
    expect(screen.getByRole("heading", { name: "team/healthy" })).toBeInTheDocument();
    expect(screen.getByLabelText("Current location")).toHaveTextContent(/^\/workflows$/);
  });

  it("restores filters from the URL and shows an empty result for unmatched searches", async () => {
    const user = userEvent.setup();
    renderPage("/workflows?status=failing&q=broken");
    await screen.findByRole("heading", { name: "team/broken" });
    expect(screen.getByRole("searchbox", { name: "Search" })).toHaveValue("broken");
    expect(screen.getByRole("combobox", { name: "Status" })).toHaveValue("failing");
    expect(screen.queryByRole("heading", { name: "team/busy" })).not.toBeInTheDocument();
    await user.type(screen.getByRole("searchbox", { name: "Search" }), "-missing");
    expect(screen.getByText("No workflows match the current filters.")).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "team/broken" })).not.toBeInTheDocument();
  });
});
