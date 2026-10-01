import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { IssueSummary } from "../../generated/contracts.ts";
import { ActivityPage } from "./ActivityPage.tsx";

const mocks = vi.hoisted(() => ({
  client: {
    listWorkflows: vi.fn(),
    listActivity: vi.fn(),
    issueDetails: vi.fn(),
    synchronizationStatus: vi.fn(),
  },
  settings: { issuesEnabled: true, pullRequestsEnabled: true },
}));
vi.mock("../../api/application-client.tsx", () => ({ useApplicationClient: () => mocks.client }));
vi.mock("../../api/monitoring-settings.ts", () => ({
  useMonitoringSettings: () => ({ settings: mocks.settings }),
}));

const issue: IssueSummary = {
  id: "issue-1",
  number: 7,
  title: "Fix build",
  author: "alice",
  state: "open",
  labels: ["bug"],
  assignees: ["bob"],
  commentCount: 1,
  createdAt: "2026-09-28T00:00:00Z",
  updatedAt: "2026-09-29T00:00:00Z",
  webUrl: "https://provider.example/team/app/issues/7",
  sourceId: "connection",
  sourceName: "GitHub",
  sourceAbbreviation: "GH",
  repositoryId: "repo",
  repositoryOwner: "team",
  repositoryName: "app",
};

describe("issue activity", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.settings.issuesEnabled = true;
    mocks.client.synchronizationStatus.mockResolvedValue(null);
    mocks.client.listWorkflows.mockResolvedValue({ selectedRepositoryCount: 1, workflows: [] });
    mocks.client.listActivity.mockResolvedValue({
      changeRequestEvents: [],
      issueEvents: [
        { id: "opened", kind: "opened", occurredAt: issue.createdAt, issue },
        {
          id: "closed",
          kind: "closed",
          occurredAt: issue.updatedAt,
          issue: { ...issue, state: "closed" },
        },
      ],
    });
    mocks.client.issueDetails.mockResolvedValue({
      title: issue.title,
      state: "closed",
      labels: issue.labels,
      assignees: issue.assignees,
      commentCount: 1,
      updatedAt: issue.updatedAt,
      body: "Build fixed",
      milestone: null,
      comments: [],
    });
  });

  it("sorts issue events, filters them, and opens current issue details", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter>
        <ActivityPage />
      </MemoryRouter>,
    );
    const rows = await screen.findAllByRole("button", { name: "Open issue 7 in team/app" });
    expect(rows).toHaveLength(2);
    expect(within(rows[0]!).getByText("Closed")).toBeInTheDocument();
    expect(within(rows[1]!).getByText("Opened")).toBeInTheDocument();
    await user.selectOptions(screen.getByRole("combobox", { name: "Type" }), "issues");
    expect(screen.getByRole("combobox", { name: "Trigger" })).toBeDisabled();
    await user.selectOptions(screen.getByRole("combobox", { name: "Status" }), "successful");
    expect(screen.getAllByRole("button", { name: "Open issue 7 in team/app" })).toHaveLength(1);
    await user.type(screen.getByRole("searchbox", { name: "Search" }), "bob");
    await user.click(screen.getByRole("button", { name: "Open issue 7 in team/app" }));
    const dialog = screen.getByRole("dialog", { name: issue.title });
    expect(await within(dialog).findByText("Build fixed")).toBeInTheDocument();
    expect(mocks.client.issueDetails).toHaveBeenCalledWith({
      sourceId: "connection",
      repositoryId: "repo",
      number: 7,
    });
    await user.click(within(dialog).getByRole("button", { name: "Close issue details" }));
    await user.clear(screen.getByRole("searchbox", { name: "Search" }));
    await user.type(screen.getByRole("searchbox", { name: "Search" }), "unrelated");
    expect(screen.getByText("No activity matches the current filters.")).toBeInTheDocument();
  });

  it("excludes issues when issue monitoring is disabled", async () => {
    mocks.settings.issuesEnabled = false;
    render(
      <MemoryRouter initialEntries={["/activity?type=issues"]}>
        <ActivityPage />
      </MemoryRouter>,
    );
    expect(await screen.findByText("No activity has been cached yet.")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Open issue 7 in team/app" }),
    ).not.toBeInTheDocument();
  });
});
