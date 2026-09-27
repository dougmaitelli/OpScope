import { useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { useApplicationClient } from "../../api/application-client.tsx";
import { EmptyState } from "../../components/EmptyState.tsx";
import { PageHeader } from "../../components/PageHeader.tsx";
import { PanelHeader } from "../../components/PanelHeader.tsx";
import { WorkflowRunLogsDialog } from "../../components/WorkflowRunLogsDialog.tsx";
import type {
  ListWorkflowsResponse,
  WorkflowRunSummary,
  WorkflowSummary,
} from "../../generated/contracts.ts";
import {
  ActivityFilters,
  type ActivityFilterValues,
  type ActivityRangeFilter,
  type ActivityStatusFilter,
} from "./ActivityFilters.tsx";
import { ActivityRow, type WorkflowActivity } from "./ActivityRow.tsx";
import "./ActivityPage.css";

const STATUS_FILTERS = new Set<ActivityStatusFilter>([
  "all",
  "failing",
  "running",
  "successful",
  "other",
]);
const RANGE_FILTERS = new Set<ActivityRangeFilter>(["all", "day", "week", "month"]);

function matchesStatus(run: WorkflowRunSummary, status: ActivityStatusFilter): boolean {
  if (status === "all") return true;
  const running = run.lifecycle === "queued" || run.lifecycle === "running";
  const failing = run.lifecycle === "completed" && run.outcome === "failure";
  const successful = run.lifecycle === "completed" && run.outcome === "success";
  if (status === "running") return running;
  if (status === "failing") return failing;
  if (status === "successful") return successful;
  return !running && !failing && !successful;
}

function rangeCutoff(range: ActivityRangeFilter): number | null {
  const day = 24 * 60 * 60 * 1000;
  if (range === "day") return Date.now() - day;
  if (range === "week") return Date.now() - 7 * day;
  if (range === "month") return Date.now() - 30 * day;
  return null;
}

export function ActivityPage() {
  const client = useApplicationClient();
  const [searchParams, setSearchParams] = useSearchParams();
  const [inventory, setInventory] = useState<ListWorkflowsResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);
  const [selected, setSelected] = useState<{
    workflow: WorkflowSummary;
    run: WorkflowRunSummary;
  } | null>(null);

  useEffect(() => {
    let active = true;
    client
      .listWorkflows()
      .then((response) => {
        if (active) setInventory(response);
      })
      .catch(() => {
        if (active) setError(true);
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [client]);

  const activities = useMemo(
    () =>
      (inventory?.workflows ?? [])
        .flatMap((workflow) =>
          workflow.runs.map((run): WorkflowActivity => ({
            id: `${workflow.sourceId}:${workflow.repositoryId}:${run.id}:${run.attempt}`,
            repositoryId: `${workflow.sourceId}:${workflow.repositoryId}`,
            repositoryLabel: `${workflow.repositoryOwner}/${workflow.repositoryName}`,
            workflow,
            run,
          })),
        )
        .sort((left, right) => Date.parse(right.run.createdAt) - Date.parse(left.run.createdAt)),
    [inventory],
  );

  const requestedStatus = searchParams.get("status") ?? "all";
  const requestedRange = searchParams.get("range") ?? "all";
  const filters: ActivityFilterValues = {
    query: searchParams.get("q") ?? "",
    status: STATUS_FILTERS.has(requestedStatus as ActivityStatusFilter)
      ? (requestedStatus as ActivityStatusFilter)
      : "all",
    repository: searchParams.get("repository") ?? "",
    trigger: searchParams.get("trigger") ?? "",
    range: RANGE_FILTERS.has(requestedRange as ActivityRangeFilter)
      ? (requestedRange as ActivityRangeFilter)
      : "all",
  };

  const repositories = useMemo(() => {
    const values = new Map<string, string>();
    for (const activity of activities) {
      values.set(activity.repositoryId, activity.repositoryLabel);
    }
    return [...values]
      .map(([id, label]) => ({ id, label }))
      .sort((left, right) => left.label.localeCompare(right.label));
  }, [activities]);
  const triggers = useMemo(
    () => [...new Set(activities.map((activity) => activity.run.trigger))].sort(),
    [activities],
  );

  const normalizedQuery = filters.query.trim().toLocaleLowerCase();
  const cutoff = rangeCutoff(filters.range);
  const filteredActivities = activities.filter((activity) => {
    const { workflow, run } = activity;
    const searchable = [
      activity.repositoryLabel,
      workflow.name,
      run.title,
      run.branch,
      run.commitSha,
      run.actor,
      run.trigger,
    ]
      .filter(Boolean)
      .join(" ")
      .toLocaleLowerCase();
    return (
      searchable.includes(normalizedQuery) &&
      matchesStatus(run, filters.status) &&
      (filters.repository.length === 0 || activity.repositoryId === filters.repository) &&
      (filters.trigger.length === 0 || run.trigger === filters.trigger) &&
      (cutoff === null || Date.parse(run.createdAt) >= cutoff)
    );
  });
  const filtersActive =
    normalizedQuery.length > 0 ||
    filters.status !== "all" ||
    filters.repository.length > 0 ||
    filters.trigger.length > 0 ||
    filters.range !== "all";

  const updateFilters = (values: ActivityFilterValues) => {
    const next = new URLSearchParams();
    if (values.query.trim().length > 0) next.set("q", values.query);
    if (values.status !== "all") next.set("status", values.status);
    if (values.repository.length > 0) next.set("repository", values.repository);
    if (values.trigger.length > 0) next.set("trigger", values.trigger);
    if (values.range !== "all") next.set("range", values.range);
    setSearchParams(next, { replace: true });
  };

  return (
    <section className="page-view" aria-labelledby="activity-title">
      <PageHeader
        eyebrow="Workflow runs"
        title="Activity"
        description="Recent activity across all monitored repositories."
      />

      <section className="panel activity-panel" aria-labelledby="activity-list-title">
        <PanelHeader
          label="Timeline"
          title="Recent workflow activity"
          metadata={
            <span className="panel-badge">
              {error
                ? "Unavailable"
                : filtersActive
                  ? `${filteredActivities.length} of ${activities.length} runs`
                  : `${activities.length} run${activities.length === 1 ? "" : "s"}`}
            </span>
          }
        />
        {activities.length > 0 ? (
          <>
            <ActivityFilters
              values={filters}
              repositories={repositories}
              triggers={triggers}
              onChange={updateFilters}
              onClear={() =>
                updateFilters({
                  query: "",
                  status: "all",
                  repository: "",
                  trigger: "",
                  range: "all",
                })
              }
            />
            <div className="activity-list-header" aria-hidden="true">
              <span>Status</span>
              <span>Run</span>
              <span>Project / workflow</span>
              <span className="activity-ref-heading">Ref</span>
              <span className="activity-actor-heading">Actor / trigger</span>
              <span />
            </div>
          </>
        ) : null}
        <div className="activity-list" aria-live="polite">
          {loading ? (
            <p className="activity-loading">Loading cached activity…</p>
          ) : error ? (
            <EmptyState message="The activity timeline could not be loaded." error />
          ) : activities.length === 0 ? (
            <EmptyState message="No workflow activity has been cached yet." />
          ) : filteredActivities.length === 0 ? (
            <EmptyState message="No workflow activity matches the current filters." />
          ) : (
            filteredActivities.map((activity) => (
              <ActivityRow
                activity={activity}
                key={activity.id}
                onOpen={() => setSelected({ workflow: activity.workflow, run: activity.run })}
              />
            ))
          )}
        </div>
      </section>

      {selected ? (
        <WorkflowRunLogsDialog
          workflow={selected.workflow}
          run={selected.run}
          onClose={() => setSelected(null)}
        />
      ) : null}
    </section>
  );
}
