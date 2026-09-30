import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { IssueSummary, IssueDetailsResponse } from "../../generated/contracts.ts";
import { IssuesPage } from "./IssuesPage.tsx";

const client = vi.hoisted(() => ({
  listIssues: vi.fn(),
  issueDetails: vi.fn(),
  synchronizeSources: vi.fn(),
}));
vi.mock("../../api/application-client.tsx", () => ({ useApplicationClient: () => client }));

describe("multi-provider issues", () => {
  it.each(["GitLab", "Gitea"])(
    "opens grouped %s issues and displays current details",
    async (sourceName) => {
      vi.clearAllMocks();
      client.synchronizeSources.mockResolvedValue({
        failedRepositoryCount: 0,
        alreadyRunning: false,
      });
      const user = userEvent.setup();
      const issue: IssueSummary = {
        id: "101",
        number: 7,
        title: "Fix build",
        author: "alice",
        state: "open",
        labels: ["bug"],
        assignees: ["bob"],
        commentCount: 1,
        createdAt: "2026-09-28T00:00:00Z",
        updatedAt: "2026-09-28T00:01:00Z",
        webUrl: "https://provider.example/team/app/issues/7",
        sourceId: "connection-1",
        sourceName,
        sourceAbbreviation: "SC",
        repositoryId: "repo-1",
        repositoryOwner: "team",
        repositoryName: "app",
      };
      const details: IssueDetailsResponse = {
        title: "Fix build",
        state: "closed",
        labels: ["bug"],
        assignees: ["bob"],
        commentCount: 1,
        updatedAt: "2026-09-28T00:02:00Z",
        body: "## Issue description",
        milestone: "Next",
        comments: [
          {
            id: "10",
            author: "bob",
            body: "**Fixed in the latest build**",
            createdAt: "2026-09-28T00:02:00Z",
            updatedAt: "2026-09-28T00:02:00Z",
          },
        ],
      };
      client.listIssues.mockResolvedValue({ selectedRepositoryCount: 1, issues: [issue] });
      client.issueDetails.mockResolvedValue(details);
      render(<IssuesPage />);
      await screen.findByRole("button", { name: /Fix build/ });
      await user.click(screen.getByRole("button", { name: "Refresh" }));
      expect(client.synchronizeSources).toHaveBeenCalledExactlyOnceWith({ scope: "issues" });
      await user.click(await screen.findByRole("button", { name: /Fix build/ }));
      const dialog = screen.getByRole("dialog", { name: "Fix build" });
      expect(
        await within(dialog).findByRole("heading", { name: "Issue description", level: 2 }),
      ).toBeInTheDocument();
      expect(within(dialog).getByText("Closed")).toBeInTheDocument();
      expect(within(dialog).getByText("Fixed in the latest build").tagName).toBe("STRONG");
      expect(within(dialog).getByRole("link", { name: `Open in ${sourceName}` })).toHaveAttribute(
        "href",
        issue.webUrl,
      );
      expect(client.issueDetails).toHaveBeenCalledWith({
        sourceId: "connection-1",
        repositoryId: "repo-1",
        number: 7,
      });
      await user.click(within(dialog).getByRole("button", { name: "Close issue details" }));
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    },
  );
});
