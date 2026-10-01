import { useFeatureRefresh } from "../../api/use-feature-refresh.ts";
import { RefreshButton } from "../../components/RefreshButton.tsx";
import { useSynchronizedData } from "../../api/use-synchronized-data.ts";
import { updateWorkflowRun } from "../../shared/action-refresh.ts";
import { useSearchParams } from "react-router-dom";
import { useApplicationClient } from "../../api/application-client.tsx";
import { EmptyState } from "../../components/EmptyState.tsx";
import { InventorySummary } from "../../components/InventorySummary.tsx";
import { PageHeader } from "../../components/PageHeader.tsx";
import { PanelHeader } from "../../components/PanelHeader.tsx";
import { ProjectGroupList } from "../../components/ProjectGroup.tsx";
import { groupByProject } from "../../shared/project-groups.ts";
import { WorkflowProjectGroup } from "./WorkflowProjectGroup.tsx";
import { WorkflowFilters, type WorkflowStatusFilter } from "../../components/WorkflowFilters.tsx";
import type { ListWorkflowsResponse, WorkflowSummary } from "../../generated/contracts.ts";
import { TimeDisplay } from "../../components/TimeDisplay.tsx";
import "./WorkflowsPage.css";

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

export function WorkflowsPage() {
  const client = useApplicationClient();
  const [searchParams, setSearchParams] = useSearchParams();
  const {
    data: inventory,
    setData: setInventory,
    error,
    setError,
    loading,
    status: syncStatus,
  } = useSynchronizedData<ListWorkflowsResponse>(() => client.listWorkflows());
  const backgroundSyncing = syncStatus?.running ?? false;
  const {
    refresh,
    refreshing,
    notice: refreshNotice,
  } = useFeatureRefresh("workflows", async () => {
    setInventory(await client.listWorkflows());
    setError(null);
  });

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
  const updated = inventory?.lastSuccessfulAt ? (
    <>
      Last synchronized <TimeDisplay dateTime={inventory.lastSuccessfulAt} />
    </>
  ) : (
    "No run activity synchronized yet"
  );

  const updateFilters = (nextQuery: string, nextStatus: WorkflowStatusFilter) => {
    const next = new URLSearchParams(searchParams);
    if (nextQuery.trim().length > 0) next.set("q", nextQuery);
    else next.delete("q");
    if (nextStatus !== "all") next.set("status", nextStatus);
    else next.delete("status");
    setSearchParams(next, { replace: true });
  };

  return (
    <section className="page-view" aria-label="Workflows">
      <PageHeader
        eyebrow="Monitored repositories"
        title="Workflows"
        description="Workflows discovered across your selected repositories."
        actions={
          <RefreshButton
            busy={refreshing || backgroundSyncing}
            disabled={loading}
            onRefresh={refresh}
          />
        }
      />

      <InventorySummary
        heading={loading && !inventory ? "Loading workflows" : heading}
        description={
          error ? (
            "Check that the application service is running"
          ) : inventory ? (
            <>
              {updated} · {inventory.selectedRepositoryCount} selected repositor
              {inventory.selectedRepositoryCount === 1 ? "y" : "ies"}
            </>
          ) : (
            "Waiting for the application core"
          )
        }
        tone={statusTone}
        busy={loading || backgroundSyncing}
        stats={[
          { label: "Workflows", value: error ? "—" : workflows.length },
          {
            label: "Running",
            value: error ? "—" : running,
            tone: "running",
            actionLabel: `Show ${running} running workflow${running === 1 ? "" : "s"}`,
            onSelect: !error && running > 0 ? () => updateFilters("", "running") : undefined,
          },
          {
            label: "Failing",
            value: error ? "—" : failing,
            tone: "failing",
            actionLabel: `Show ${failing} failing workflow${failing === 1 ? "" : "s"}`,
            onSelect: !error && failing > 0 ? () => updateFilters("", "failing") : undefined,
          },
        ]}
      />

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

      <div className="workflows-content">
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
              projects.map((project) => (
                <WorkflowProjectGroup
                  key={project.id}
                  project={project}
                  onRunUpdated={(workflow, run) =>
                    setInventory(
                      (current) =>
                        current && {
                          ...current,
                          workflows: updateWorkflowRun(current.workflows, workflow, run),
                        },
                    )
                  }
                />
              ))
            )}
          </ProjectGroupList>
        </section>
      </div>
    </section>
  );
}
