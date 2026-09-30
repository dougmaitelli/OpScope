import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { ChangeRequestSummary } from "../../generated/contracts.ts";
import { ChangeRequestsPage } from "./ChangeRequestsPage.tsx";

const client = vi.hoisted(() => ({
  listChangeRequests: vi.fn(),
  changeRequestDetails: vi.fn(),
  listWorkflows: vi.fn(),
}));
vi.mock("../../api/application-client.tsx", () => ({
  useApplicationClient: () => client,
}));

describe("multi-provider pull requests", () => {
  it.each(["GitLab", "Gitea", "Bitbucket Cloud"])(
    "opens grouped %s requests through the shared details dialog",
    async (sourceName) => {
      const user = userEvent.setup();
      const request: ChangeRequestSummary = {
        id: "42",
        number: 7,
        title: "Improve build checks",
        author: "alice",
        sourceBranch: "feature",
        targetBranch: "main",
        state: "open",
        draft: false,
        reviewStatus: "unknown",
        checkStatus: "unknown",
        mergeStatus: "unknown",
        createdAt: "2026-09-01T00:00:00Z",
        updatedAt: "2026-09-01T00:00:00Z",
        webUrl: "https://provider.example/team/project/pulls/7",
        sourceId: "connection-1",
        sourceName,
        sourceAbbreviation: "SC",
        repositoryId: "repo-1",
        repositoryOwner: "team",
        repositoryName: "project",
      };
      client.listChangeRequests.mockResolvedValue({
        selectedRepositoryCount: 1,
        changeRequests: [request],
      });
      client.listWorkflows.mockResolvedValue({ workflows: [] });
      client.changeRequestDetails.mockResolvedValue({
        body: "## Provider pull request description\n\n<details><summary>Release notes</summary><p>A formatted update.</p></details>",
        labels: [],
        reviews: [],
        checks: [
          {
            name: "External build",
            status: "passed",
            webUrl: "https://ci.example/build/1",
            workflowRunId: null,
          },
        ],
        latestCommit: null,
      });
      render(<ChangeRequestsPage />);
      const row = await screen.findByRole("button", { name: /Improve build checks/ });
      expect(within(row).getByText("Checks unknown")).toBeInTheDocument();
      expect(within(row).getByText("Review unknown")).toBeInTheDocument();
      await user.click(row);
      const dialog = screen.getByRole("dialog", { name: request.title });
      expect(
        await within(dialog).findByRole("heading", {
          name: "Provider pull request description",
          level: 2,
        }),
      ).toBeInTheDocument();
      expect(within(dialog).getByText("Release notes").tagName).toBe("SUMMARY");
      expect(client.changeRequestDetails).toHaveBeenCalledWith({
        sourceId: "connection-1",
        repositoryId: "repo-1",
        number: 7,
      });
      expect(within(dialog).getByRole("link", { name: `Open in ${sourceName}` })).toHaveAttribute(
        "href",
        request.webUrl,
      );
      expect(within(dialog).getByRole("link", { name: /External build/ })).toHaveAttribute(
        "href",
        "https://ci.example/build/1",
      );
      await user.click(within(dialog).getByRole("button", { name: "Close pull request details" }));
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    },
  );
});
