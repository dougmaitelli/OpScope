import { useCallback, useEffect, useState } from "react";
import { useApplicationClient } from "../../api/application-client.tsx";
import { EmptyState } from "../../components/EmptyState.tsx";
import { PageHeader } from "../../components/PageHeader.tsx";
import { PanelHeader } from "../../components/PanelHeader.tsx";
import { groupWorkflows, ProjectGroup } from "../../components/ProjectGroup.tsx";
import type { ListWorkflowsResponse } from "../../generated/contracts.ts";
import { formatRelativeUnix } from "../../shared/workflow-runs.ts";
import "./OverviewPage.css";

export function OverviewPage() {
  const client = useApplicationClient();
  const [inventory, setInventory] = useState<ListWorkflowsResponse | null>(null);
  const [error, setError] = useState(false);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [refreshNotice, setRefreshNotice] = useState<string | null>(null);

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

  const workflows = inventory?.workflows ?? [];
  const projects = groupWorkflows(workflows);
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
  const statusTone = error || failing > 0
    ? "failing"
    : running > 0
      ? "running"
      : workflows.length > 0
        ? "passing"
        : "idle";
  const statusIcon = statusTone === "failing"
    ? "!"
    : statusTone === "running"
      ? "↻"
      : statusTone === "passing"
        ? "✓"
        : "·";
  const updated = inventory?.lastSuccessfulAt
    ? `Last synchronized ${formatRelativeUnix(inventory.lastSuccessfulAt)}`
    : "No run activity synchronized yet";

  return (
    <section className="page-view" aria-labelledby="overview-title">
      <PageHeader
        eyebrow="Monitored repositories"
        title="Overview"
        description="Workflows discovered across your selected repositories."
        actions={
          <button
            className={`secondary-button${loading ? " button-busy" : ""}`}
            type="button"
            disabled={loading}
            onClick={() => void refresh()}
          >
            <span aria-hidden="true">↻</span>
            {refreshing ? "Synchronizing…" : "Refresh"}
          </button>
        }
      />

      <section className="health-summary" aria-labelledby="health-heading" aria-busy={loading}>
        <div className="health-heading">
          <span
            className={`health-icon health-icon-${statusTone}`}
            aria-hidden="true"
          >
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
          <div><dt>Workflows</dt><dd>{error ? "—" : workflows.length}</dd></div>
          <div><dt>Running</dt><dd>{error ? "—" : running}</dd></div>
          <div><dt>Failing</dt><dd>{error ? "—" : failing}</dd></div>
        </dl>
      </section>

      {inventory?.stale ? (
        <div className="sync-warning" role="status">
          <strong>Live refresh failed.</strong> Cached workflow activity is still available.
          {inventory.syncError ? ` ${inventory.syncError}` : ""}
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
            metadata={<span className="panel-badge">{error ? "Unavailable" : `${projects.length} project${projects.length === 1 ? "" : "s"}`}</span>}
          />
          <div className="workflow-list" aria-live="polite">
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
            ) : (
              projects.map((project) => <ProjectGroup key={project.id} project={project} />)
            )}
          </div>
        </section>
      </div>
    </section>
  );
}
