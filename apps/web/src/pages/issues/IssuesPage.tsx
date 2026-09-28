import { useEffect, useMemo, useState } from "react";
import { useApplicationClient } from "../../api/application-client.tsx";
import { EmptyState } from "../../components/EmptyState.tsx";
import { PageHeader } from "../../components/PageHeader.tsx";
import { PanelHeader } from "../../components/PanelHeader.tsx";
import type { IssueSummary, ListIssuesResponse } from "../../generated/contracts.ts";
import { requestErrorMessage } from "../../shared/errors.ts";
import { IssueDetailsDialog } from "./IssueDetailsDialog.tsx";
import { IssueFilters } from "./IssueFilters.tsx";
import { IssueProjectGroup } from "./IssueProjectGroup.tsx";
import { ProjectGroupList } from "../../components/ProjectGroup.tsx";
import { groupByProject } from "../../shared/project-groups.ts";
import "./IssuesPage.css";

const ISSUE_REFRESH_INTERVAL_MS = 10_000;

export function IssuesPage() {
  const client = useApplicationClient();
  const [inventory, setInventory] = useState<ListIssuesResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [repository, setRepository] = useState("all");
  const [assignment, setAssignment] = useState("all");
  const [refreshing, setRefreshing] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [selected, setSelected] = useState<IssueSummary | null>(null);

  useEffect(() => {
    let active = true;
    const load = () =>
      client
        .listIssues()
        .then((response) => {
          if (active) {
            setInventory(response);
            setError(null);
          }
        })
        .catch((failure: unknown) => {
          if (active) setError(requestErrorMessage(failure));
        })
        .finally(() => {
          if (active) setLoading(false);
        });
    void load();
    const interval = window.setInterval(() => void load(), ISSUE_REFRESH_INTERVAL_MS);
    return () => {
      active = false;
      window.clearInterval(interval);
    };
  }, [client]);

  async function refresh() {
    setRefreshing(true);
    setNotice(null);
    try {
      const result = await client.synchronizeSources();
      setInventory(await client.listIssues());
      setError(null);
      if (result.failedRepositoryCount > 0) {
        setNotice(
          "Some repositories could not be refreshed. Previously loaded issues remain available.",
        );
      } else if (result.alreadyRunning) {
        setNotice("Synchronization is already running. Refresh again once it finishes.");
      }
    } catch (failure) {
      setNotice(requestErrorMessage(failure));
    } finally {
      setRefreshing(false);
    }
  }

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

  return (
    <section className="page-view" aria-label="Issues">
      <PageHeader
        eyebrow="Tracked work"
        title="Issues"
        description="Open issues across monitored repositories, with ownership and recent discussion."
        actions={
          <button
            className="secondary-button"
            type="button"
            disabled={refreshing}
            aria-busy={refreshing}
            onClick={() => void refresh()}
          >
            {refreshing ? "Refreshing…" : "Refresh"}
          </button>
        }
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
