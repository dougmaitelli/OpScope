import { useFeatureRefresh } from "../../api/use-feature-refresh.ts";
import { RefreshButton } from "../../components/RefreshButton.tsx";
import { useSynchronizedData } from "../../api/use-synchronized-data.ts";
import { useMemo, useState } from "react";
import { useApplicationClient } from "../../api/application-client.tsx";
import { EmptyState } from "../../components/EmptyState.tsx";
import { PageHeader } from "../../components/PageHeader.tsx";
import { PanelHeader } from "../../components/PanelHeader.tsx";
import { InventorySummary } from "../../components/InventorySummary.tsx";
import type { IssueSummary, ListIssuesResponse } from "../../generated/contracts.ts";
import { IssueDetailsDialog } from "./IssueDetailsDialog.tsx";
import { IssueFilters } from "./IssueFilters.tsx";
import { IssueProjectGroup } from "./IssueProjectGroup.tsx";
import { ProjectGroupList } from "../../components/ProjectGroup.tsx";
import { groupByProject } from "../../shared/project-groups.ts";
import "./IssuesPage.css";

export function IssuesPage() {
  const client = useApplicationClient();
  const {
    data: inventory,
    setData: setInventory,
    loading,
    error,
    setError,
  } = useSynchronizedData<ListIssuesResponse>(() => client.listIssues());
  const [query, setQuery] = useState("");
  const [repository, setRepository] = useState("all");
  const [assignment, setAssignment] = useState("all");
  const [selected, setSelected] = useState<IssueSummary | null>(null);

  const { refresh, refreshing, notice } = useFeatureRefresh("issues", async () => {
    setInventory(await client.listIssues());
    setError(null);
  });

  const issues = useMemo(() => inventory?.issues ?? [], [inventory]);
  const repositories = useMemo(() => {
    const options = new Map<string, string>();
    for (const issue of issues) {
      const id = `${issue.sourceId}:${issue.repositoryId}`;
      options.set(id, `${issue.repositoryOwner}/${issue.repositoryName} · ${issue.sourceName}`);
    }
    return [...options]
      .map(([id, label]) => ({ id, label }))
      .sort((a, b) => a.label.localeCompare(b.label));
  }, [issues]);
  const filtered = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    return issues.filter((issue) => {
      const repositoryId = `${issue.sourceId}:${issue.repositoryId}`;
      const searchable = [
        issue.title,
        `#${issue.number}`,
        issue.author,
        issue.repositoryOwner,
        issue.repositoryName,
        ...issue.labels,
        ...issue.assignees,
      ]
        .filter(Boolean)
        .join(" ")
        .toLocaleLowerCase();
      return (
        (repository === "all" || repository === repositoryId) &&
        searchable.includes(normalized) &&
        (assignment === "all" ||
          (assignment === "assigned" ? issue.assignees.length > 0 : issue.assignees.length === 0))
      );
    });
  }, [issues, query, repository, assignment]);

  const projects = useMemo(() => groupByProject(filtered), [filtered]);
  const assigned = issues.filter((issue) => issue.assignees.length > 0).length;
  const unassigned = issues.length - assigned;
  const unavailable = !!error || !inventory;
  const selectAssignment = (nextAssignment: string) => {
    setQuery("");
    setRepository("all");
    setAssignment(nextAssignment);
  };

  return (
    <section className="page-view" aria-label="Issues">
      <PageHeader
        eyebrow="Tracked work"
        title="Issues"
        description="Open issues across monitored repositories, with ownership and recent discussion."
        actions={<RefreshButton busy={refreshing} disabled={loading} onRefresh={refresh} />}
      />
      <InventorySummary
        heading={
          error
            ? "Unable to load issues"
            : loading && !inventory
              ? "Loading issues"
              : `${issues.length} open issue${issues.length === 1 ? "" : "s"}`
        }
        description={
          error
            ? "Check that the application service is running"
            : inventory
              ? `${inventory.selectedRepositoryCount} selected repositor${inventory.selectedRepositoryCount === 1 ? "y" : "ies"}`
              : "Waiting for the application core"
        }
        tone={error ? "failing" : "idle"}
        busy={loading || refreshing}
        stats={[
          {
            label: "Open issues",
            value: unavailable ? "—" : issues.length,
            actionLabel: "Show all open issues",
            onSelect: !unavailable && issues.length > 0 ? () => selectAssignment("all") : undefined,
          },
          {
            label: "Assigned",
            value: unavailable ? "—" : assigned,
            actionLabel: `Show ${assigned} assigned issue${assigned === 1 ? "" : "s"}`,
            onSelect: !unavailable && assigned > 0 ? () => selectAssignment("assigned") : undefined,
          },
          {
            label: "Unassigned",
            value: unavailable ? "—" : unassigned,
            actionLabel: `Show ${unassigned} unassigned issue${unassigned === 1 ? "" : "s"}`,
            onSelect:
              !unavailable && unassigned > 0 ? () => selectAssignment("unassigned") : undefined,
          },
        ]}
      />
      {notice ? (
        <p className="issue-refresh-notice" role="status">
          {notice}
        </p>
      ) : null}
      <section className="panel issues-panel" aria-label="Open issues">
        <PanelHeader
          label="Open work"
          title="Issues"
          metadata={
            <span className="panel-badge">
              {error ? "Unavailable" : `${filtered.length} of ${issues.length}`}
            </span>
          }
        />
        {issues.length > 0 ? (
          <>
            <IssueFilters
              query={query}
              repository={repository}
              assignment={assignment}
              repositories={repositories}
              onQueryChange={setQuery}
              onRepositoryChange={setRepository}
              onAssignmentChange={setAssignment}
            />
          </>
        ) : null}
        <ProjectGroupList className="issue-list" aria-live="polite">
          {loading ? (
            <p className="issue-loading">Loading cached issues…</p>
          ) : error ? (
            <EmptyState message={error} error />
          ) : (inventory?.selectedRepositoryCount ?? 0) === 0 ? (
            <EmptyState message="Select repositories to monitor before loading issues." />
          ) : issues.length === 0 ? (
            <EmptyState message="No open issues were found in the monitored repositories." />
          ) : filtered.length === 0 ? (
            <EmptyState message="No issues match the current filters." />
          ) : (
            projects.map((project) => (
              <IssueProjectGroup key={project.id} project={project} onOpen={setSelected} />
            ))
          )}
        </ProjectGroupList>
      </section>
      {selected ? <IssueDetailsDialog issue={selected} onClose={() => setSelected(null)} /> : null}
    </section>
  );
}
