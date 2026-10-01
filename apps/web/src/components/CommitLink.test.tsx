import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { CommitLink } from "./CommitLink.tsx";
import { ActivityRow } from "../pages/activity/ActivityRow.tsx";
import type { WorkflowRunSummary, WorkflowSummary } from "../generated/contracts.ts";

const sha = "abcdef1234567890";

describe("commit links", () => {
  it.each([
    ["https://github.com/team/app/actions/runs/12", "https://github.com/team/app/commit/"],
    [
      "https://github.example/prefix/team/app/pull/7",
      "https://github.example/prefix/team/app/commit/",
    ],
    ["https://gitea.example/team/app/pulls/7", "https://gitea.example/team/app/commit/"],
    [
      "https://gitlab.example/team/subgroup/app/-/pipelines/12?ref=main#jobs",
      "https://gitlab.example/team/subgroup/app/-/commit/",
    ],
    [
      "https://gitlab.example/team/app/-/merge_requests/7",
      "https://gitlab.example/team/app/-/commit/",
    ],
    [
      "https://bitbucket.org/team/app/pipelines/results/12",
      "https://bitbucket.org/team/app/commits/",
    ],
    ["https://bitbucket.org/team/app/pull-requests/7", "https://bitbucket.org/team/app/commits/"],
    [
      "https://bitbucket.org/team/app/addon/pipelines/home#!/results/12",
      "https://bitbucket.org/team/app/commits/",
    ],
  ])("links %s to its commit", (resourceUrl, expectedPrefix) => {
    render(<CommitLink sha={sha} resourceUrl={resourceUrl} />);
    const link = screen.getByRole("link", { name: `Open commit ${sha}` });
    expect(link).toHaveAttribute("href", `${expectedPrefix}${sha}`);
    expect(link).toHaveTextContent("abcdef1");
    expect(link).toHaveAttribute("title", sha);
    expect(link).toHaveAttribute("target", "_blank");
  });

  it.each(["not a URL", "javascript:alert(1)", "https://example.com/unknown"])(
    "keeps the SHA readable for incomplete metadata: %s",
    (resourceUrl) => {
      render(<CommitLink sha={sha} resourceUrl={resourceUrl} />);
      expect(screen.getByText("abcdef1")).toHaveAttribute("title", sha);
      expect(screen.queryByRole("link")).not.toBeInTheDocument();
    },
  );

  it("keeps link interactions separate from opening the activity row", async () => {
    const user = userEvent.setup();
    const onOpen = vi.fn();
    const run: WorkflowRunSummary = {
      id: "12",
      runNumber: 12,
      attempt: 1,
      title: "Build",
      lifecycle: "completed",
      outcome: "success",
      branch: "main",
      commitSha: sha,
      actor: "alice",
      trigger: "push",
      createdAt: "2026-09-30T10:00:00Z",
      startedAt: null,
      updatedAt: "2026-09-30T10:01:00Z",
      webUrl: "https://github.com/team/app/actions/runs/12",
    };
    const workflow: WorkflowSummary = {
      id: "build",
      name: "Build",
      path: ".github/workflows/build.yml",
      state: "active",
      webUrl: "https://github.com/team/app/actions",
      sourceId: "source",
      sourceName: "GitHub",
      sourceAbbreviation: "GH",
      repositoryId: "app",
      repositoryOwner: "team",
      repositoryName: "app",
      runs: [run],
    };
    render(
      <ActivityRow
        activity={{
          type: "workflow",
          id: "event",
          occurredAt: run.createdAt,
          repositoryId: "source:app",
          repositoryLabel: "team/app",
          workflow,
          run,
        }}
        onOpen={onOpen}
      />,
    );
    const row = screen.getByRole("button", { name: "Open logs for team/app, Build, run 12" });
    const link = screen.getByRole("link", { name: `Open commit ${sha}` });
    expect(link.closest("button")).toBeNull();
    await user.click(link);
    link.focus();
    await user.keyboard("{Enter}");
    expect(onOpen).not.toHaveBeenCalled();
    row.focus();
    await user.keyboard("{Enter}");
    await user.keyboard(" ");
    expect(onOpen).toHaveBeenCalledTimes(2);
  });
});
