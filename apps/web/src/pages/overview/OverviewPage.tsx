import { useCallback, useEffect, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { useApplicationClient } from "../../api/application-client.tsx";
import { EmptyState } from "../../components/EmptyState.tsx";
import { PageHeader } from "../../components/PageHeader.tsx";
import { PanelHeader } from "../../components/PanelHeader.tsx";
import { ProjectGroupList } from "../../components/ProjectGroup.tsx";
import { groupByProject } from "../../shared/project-groups.ts";
import { WorkflowProjectGroup } from "./WorkflowProjectGroup.tsx";
import { WorkflowFilters, type WorkflowStatusFilter } from "../../components/WorkflowFilters.tsx";
import type { ListWorkflowsResponse, WorkflowSummary } from "../../generated/contracts.ts";
import { formatRelativeUnix } from "../../shared/workflow-runs.ts";
import "./OverviewPage.css";

const SYNCHRONIZATION_STATUS_POLL_INTERVAL_MS = 10_000;
const WORKFLOW_STATUS_FILTERS = new Set<WorkflowStatusFilter>([
  "all",
  "failing",
  "running",
  "successful",
  "other",
]);

function workflowMatchesStatus(workflow: WorkflowSummary, status: WorkflowStatusFilter): boolean {
  if (status === "all") return true;
  const latestRun = workflow.runs[0] ?? null;
  const running = latestRun?.lifecycle === "queued" || latestRun?.lifecycle === "running";
  const failing = latestRun?.lifecycle === "completed" && latestRun.outcome === "failure";
  const successful = latestRun?.lifecycle === "completed" && latestRun.outcome === "success";

  if (status === "running") return running;
  if (status === "failing") return failing;
  if (status === "successful") return successful;
  return !running && !failing && !successful;
}

export function OverviewPage() {
  const client = useApplicationClient();
  const [searchParams, setSearchParams] = useSearchParams();
  const [inventory, setInventory] = useState<ListWorkflowsResponse | null>(null);
  const [error, setError] = useState(false);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [backgroundSyncing, setBackgroundSyncing] = useState(false);
  const [refreshNotice, setRefreshNotice] = useState<string | null>(null);
  const lastObservedSynchronization = useRef<number | null | undefined>(undefined);

  const load = useCallback(async () => {
    setLoading(true);
    setError(false);
    try {
      setInventory(await client.listWorkflows());
    } catch {
      setError(true);
    } finally {
      setLoading(false);
    }
  }, [client]);

  useEffect(() => {
    void load();
  }, [load]);

  const refresh = useCallback(async () => {
    setLoading(true);
    setRefreshing(true);
    setError(false);
    setRefreshNotice(null);
    try {
      const summary = await client.synchronizeSources();
      setInventory(await client.listWorkflows());
      if (summary.failedRepositoryCount > 0) {
        setRefreshNotice(
          `${summary.failedRepositoryCount} repositor${summary.failedRepositoryCount === 1 ? "y" : "ies"} could not be refreshed. Cached data remains available where possible.`,
        );
      } else if (summary.alreadyRunning) {
        setRefreshNotice("A synchronization is already running for the selected source.");
      }
    } catch {
      try {
        setInventory(await client.listWorkflows());
        setRefreshNotice("Synchronization failed. The last available data is still displayed.");
      } catch {
        setError(true);
      }
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }, [client]);

  useEffect(() => {
    let stopped = false;
    let polling = false;
    const poll = async () => {
      if (polling) return;
      polling = true;
      try {
        const status = await client.synchronizationStatus();
        if (stopped) return;
        setBackgroundSyncing(status.running);
        const previous = lastObservedSynchronization.current;
        lastObservedSynchronization.current = status.lastCompletedAt;
        if (
          previous !== undefined &&
          status.lastCompletedAt !== null &&
          status.lastCompletedAt !== previous
        ) {
          const updatedInventory = await client.listWorkflows();
          if (!stopped) {
            setInventory(updatedInventory);
            setError(false);
            setRefreshNotice(
              status.lastFailedRepositoryCount > 0
                ? `${status.lastFailedRepositoryCount} repositor${status.lastFailedRepositoryCount === 1 ? "y" : "ies"} could not be refreshed. Cached data remains available where possible.`
                : null,
            );
          }
        }
      } catch {
        // Background polling must not replace already useful dashboard data.
      } finally {
        polling = false;
      }
    };
    void poll();
    const interval = window.setInterval(() => void poll(), SYNCHRONIZATION_STATUS_POLL_INTERVAL_MS);
    return () => {
      stopped = true;
      window.clearInterval(interval);
    };
  }, [client]);

  const workflows = inventory?.workflows ?? [];
  const query = searchParams.get("q") ?? "";
  const requestedStatus = searchParams.get("status") ?? "all";
  const status = WORKFLOW_STATUS_FILTERS.has(requestedStatus as WorkflowStatusFilter)
    ? (requestedStatus as WorkflowStatusFilter)
    : "all";
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const filteredWorkflows = workflows.filter((workflow) => {
    const searchable =
      `${workflow.repositoryOwner}/${workflow.repositoryName} ${workflow.name}`.toLocaleLowerCase();
    return searchable.includes(normalizedQuery) && workflowMatchesStatus(workflow, status);
  });
  const projects = groupByProject(filteredWorkflows);
  const filtersActive = normalizedQuery.length > 0 || status !== "all";
  const latestRuns = workflows.flatMap((workflow) => workflow.runs.slice(0, 1));
  const running = latestRuns.filter(
    (run) => run.lifecycle === "queued" || run.lifecycle === "running",
  ).length;
  const failing = latestRuns.filter(
    (run) => run.lifecycle === "completed" && run.outcome === "failure",
  ).length;
  const heading = error
    ? "Unable to load workflow activity"
    : inventory?.stale
      ? "Showing cached workflow activity"
      : failing > 0
        ? `${failing} workflow${failing === 1 ? " is" : "s are"} failing`
        : running > 0
          ? `${running} workflow${running === 1 ? " is" : "s are"} in progress`
          : latestRuns.length > 0
            ? "Latest workflow runs are healthy"
            : workflows.length > 0
              ? "Workflows are ready for their first run"
              : (inventory?.selectedRepositoryCount ?? 0) > 0
                ? "No workflows discovered"
                : "Select repositories to begin";
  const statusTone =
    error || failing > 0
      ? "failing"
      : running > 0
        ? "running"
        : workflows.length > 0
          ? "passing"
          : "idle";
  const statusIcon =
    statusTone === "failing"
      ? "!"
      : statusTone === "running"
        ? "↻"
        : statusTone === "passing"
          ? "✓"
          : "·";
  const updated = inventory?.lastSuccessfulAt
    ? `Last synchronized ${formatRelativeUnix(inventory.lastSuccessfulAt)}`
    : "No run activity synchronized yet";

  const updateFilters = (nextQuery: string, nextStatus: WorkflowStatusFilter) => {
    const next = new URLSearchParams(searchParams);
    if (nextQuery.trim().length > 0) next.set("q", nextQuery);
    else next.delete("q");
    if (nextStatus !== "all") next.set("status", nextStatus);
    else next.delete("status");
    setSearchParams(next, { replace: true });
  };

  return (
    <section className="page-view" aria-labelledby="overview-title">
      <PageHeader
        eyebrow="Monitored repositories"
        title="Overview"
        description="Workflows discovered across your selected repositories."
        actions={
          <button
            className={`secondary-button${loading || backgroundSyncing ? " button-busy" : ""}`}
            type="button"
            disabled={loading || backgroundSyncing}
            onClick={() => void refresh()}
          >
            <span aria-hidden="true">↻</span>
            {refreshing || backgroundSyncing ? "Synchronizing…" : "Refresh"}
          </button>
        }
      />

      <section
        className="health-summary"
        aria-labelledby="health-heading"
        aria-busy={loading || backgroundSyncing}
      >
        <div className="health-heading">
          <span className={`health-icon health-icon-${statusTone}`} aria-hidden="true">
            {statusIcon}
          </span>
          <div>
            <p className="section-label">Workflow inventory</p>
            <h2 id="health-heading">{loading && !inventory ? "Loading workflows" : heading}</h2>
            <p>
              {error
                ? "Check that the application service is running"
                : inventory
                  ? `${updated} · ${inventory.selectedRepositoryCount} selected repositor${inventory.selectedRepositoryCount === 1 ? "y" : "ies"}`
                  : "Waiting for the application core"}
            </p>
          </div>
        </div>
        <dl className="health-stats">
          <div>
            <dt>Workflows</dt>
            <dd>{error ? "—" : workflows.length}</dd>
          </div>
          <div>
            <dt>Running</dt>
            <dd>
              {!error && running > 0 ? (
                <button
                  className="health-stat-link health-stat-link-running"
                  type="button"
                  aria-label={`Show ${running} running workflow${running === 1 ? "" : "s"}`}
                  onClick={() => updateFilters("", "running")}
                >
                  {running}
                </button>
              ) : error ? (
                "—"
              ) : (
                running
              )}
            </dd>
          </div>
          <div>
            <dt>Failing</dt>
            <dd>
              {!error && failing > 0 ? (
                <button
                  className="health-stat-link health-stat-link-failing"
                  type="button"
                  aria-label={`Show ${failing} failing workflow${failing === 1 ? "" : "s"}`}
                  onClick={() => updateFilters("", "failing")}
                >
                  {failing}
                </button>
              ) : error ? (
                "—"
              ) : (
                failing
              )}
            </dd>
          </div>
        </dl>
      </section>

      {inventory?.stale ? (
        <div className="sync-warning" role="status">
          <strong>{inventory.syncError ? "Live refresh failed." : "Refresh pending."}</strong>
          {inventory.syncError
            ? ` Cached workflow activity is still available. ${inventory.syncError}`
            : " Cached workflow activity is displayed while background synchronization runs."}
        </div>
      ) : null}

      {refreshNotice ? (
        <div className="sync-warning" role="status">
          {refreshNotice}
        </div>
      ) : null}

      <div className="overview-content">
        <section className="panel workflows-panel" aria-labelledby="workflows-heading">
          <PanelHeader
            label="Workflows"
            title="Projects and workflows"
            metadata={
              <span className="panel-badge">
                {error
                  ? "Unavailable"
                  : filtersActive
                    ? `${filteredWorkflows.length} of ${workflows.length} workflows`
                    : `${projects.length} project${projects.length === 1 ? "" : "s"}`}
              </span>
            }
          />
          {workflows.length > 0 ? (
            <WorkflowFilters
              query={query}
              status={status}
              onQueryChange={(nextQuery) => updateFilters(nextQuery, status)}
              onStatusChange={(nextStatus) => updateFilters(query, nextStatus)}
              onClear={() => updateFilters("", "all")}
            />
          ) : null}
          <ProjectGroupList className="workflow-list" aria-live="polite">
            {loading && !inventory ? (
              <div className="loading-row">
                <span className="loading-block loading-avatar" />
                <span className="loading-block loading-copy" />
              </div>
            ) : error ? (
              <EmptyState message="The dashboard could not reach the application core." error />
            ) : !inventory || inventory.selectedRepositoryCount === 0 ? (
              <EmptyState message="Select repositories to discover workflows." />
            ) : workflows.length === 0 ? (
              <EmptyState message="No workflows were found in the selected repositories." />
            ) : filteredWorkflows.length === 0 ? (
              <EmptyState message="No workflows match the current filters." />
            ) : (
              projects.map((project) => <WorkflowProjectGroup key={project.id} project={project} />)
            )}
          </ProjectGroupList>
        </section>
      </div>
    </section>
  );
}
