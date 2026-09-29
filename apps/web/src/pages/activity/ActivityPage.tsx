import { useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { useApplicationClient } from "../../api/application-client.tsx";
import { EmptyState } from "../../components/EmptyState.tsx";
import { PageHeader } from "../../components/PageHeader.tsx";
import { PanelHeader } from "../../components/PanelHeader.tsx";
import { WorkflowRunLogsDialog } from "../../components/WorkflowRunLogsDialog.tsx";
import { ChangeRequestDetailsDialog } from "../change-requests/ChangeRequestDetailsDialog.tsx";
import type {
  ChangeRequestActivitySummary,
  ListActivityResponse,
  ListWorkflowsResponse,
  WorkflowRunSummary,
  WorkflowSummary,
} from "../../generated/contracts.ts";
import {
  ActivityFilters,
  type ActivityFilterValues,
  type ActivityRangeFilter,
  type ActivityStatusFilter,
  type ActivityTypeFilter,
} from "./ActivityFilters.tsx";
import { ActivityListHeader, ActivityRow, type WorkflowActivity } from "./ActivityRow.tsx";
import { ChangeRequestActivityRow } from "./ChangeRequestActivityRow.tsx";
import "./ActivityPage.css";
import { useMonitoringSettings } from "../../api/monitoring-settings.ts";

const STATUS_FILTERS = new Set<ActivityStatusFilter>([
  "all",
  "failing",
  "running",
  "successful",
  "other",
]);
const RANGE_FILTERS = new Set<ActivityRangeFilter>(["all", "day", "week", "month"]);
const TYPE_FILTERS = new Set<ActivityTypeFilter>(["all", "workflows", "pullRequests"]);

type TimelineActivity =
  | WorkflowActivity
  | {
      type: "pullRequest";
      id: string;
      occurredAt: string;
      repositoryId: string;
      repositoryLabel: string;
      event: ChangeRequestActivitySummary;
    };

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

function matchesChangeRequestStatus(
  event: ChangeRequestActivitySummary,
  status: ActivityStatusFilter,
): boolean {
  if (status === "all") return true;
  const failing = new Set(["changesRequested", "checksFailed", "conflictDetected"]).has(event.kind);
  const successful = new Set([
    "reviewApproved",
    "checksRecovered",
    "conflictResolved",
    "merged",
  ]).has(event.kind);
  if (status === "failing") return failing;
  if (status === "successful") return successful;
  if (status === "running") return false;
  return !failing && !successful;
}

function rangeCutoff(range: ActivityRangeFilter): number | null {
  const day = 24 * 60 * 60 * 1000;
  if (range === "day") return Date.now() - day;
  if (range === "week") return Date.now() - 7 * day;
  if (range === "month") return Date.now() - 30 * day;
  return null;
}

export function ActivityPage() {
  const { settings } = useMonitoringSettings();
  const client = useApplicationClient();
  const [searchParams, setSearchParams] = useSearchParams();
  const [inventory, setInventory] = useState<ListWorkflowsResponse | null>(null);
  const [persistedActivity, setPersistedActivity] = useState<ListActivityResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);
  const [selected, setSelected] = useState<{
    workflow: WorkflowSummary;
    run: WorkflowRunSummary;
  } | null>(null);
  const [selectedChangeRequest, setSelectedChangeRequest] =
    useState<ChangeRequestActivitySummary | null>(null);

  useEffect(() => {
    let active = true;
    Promise.all([client.listWorkflows(), client.listActivity()])
      .then(([workflowResponse, activityResponse]) => {
        if (active) {
          setInventory(workflowResponse);
          setPersistedActivity(activityResponse);
        }
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

  const activities = useMemo(() => {
    const workflows: WorkflowActivity[] = (inventory?.workflows ?? []).flatMap((workflow) =>
      workflow.runs.map((run): WorkflowActivity => ({
        type: "workflow",
        id: `${workflow.sourceId}:${workflow.repositoryId}:${run.id}:${run.attempt}`,
        occurredAt: run.createdAt,
        repositoryId: `${workflow.sourceId}:${workflow.repositoryId}`,
        repositoryLabel: `${workflow.repositoryOwner}/${workflow.repositoryName}`,
        workflow,
        run,
      })),
    );
    const changeRequests: TimelineActivity[] = (
      settings?.pullRequestsEnabled ? (persistedActivity?.changeRequestEvents ?? []) : []
    ).map((event) => ({
      type: "pullRequest",
      id: event.id,
      occurredAt: event.occurredAt,
      repositoryId: `${event.changeRequest.sourceId}:${event.changeRequest.repositoryId}`,
      repositoryLabel: `${event.changeRequest.repositoryOwner}/${event.changeRequest.repositoryName}`,
      event,
    }));
    return ([...workflows, ...changeRequests] as TimelineActivity[]).sort(
      (left, right) => Date.parse(right.occurredAt) - Date.parse(left.occurredAt),
    );
  }, [inventory, persistedActivity, settings?.pullRequestsEnabled]);

  const requestedStatus = searchParams.get("status") ?? "all";
  const requestedRange = searchParams.get("range") ?? "all";
  const requestedType = searchParams.get("type") ?? "all";
  const filters: ActivityFilterValues = {
    query: searchParams.get("q") ?? "",
    type:
      TYPE_FILTERS.has(requestedType as ActivityTypeFilter) &&
      !(requestedType === "pullRequests" && !settings?.pullRequestsEnabled)
        ? (requestedType as ActivityTypeFilter)
        : "all",
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
    () =>
      [
        ...new Set(
          activities
            .filter((activity): activity is WorkflowActivity => activity.type === "workflow")
            .map((activity) => activity.run.trigger),
        ),
      ].sort(),
    [activities],
  );

  const normalizedQuery = filters.query.trim().toLocaleLowerCase();
  const cutoff = rangeCutoff(filters.range);
  const filteredActivities = activities.filter((activity) => {
    const searchable = (
      activity.type === "workflow"
        ? [
            activity.repositoryLabel,
            activity.workflow.name,
            activity.run.title,
            activity.run.branch,
            activity.run.commitSha,
            activity.run.actor,
            activity.run.trigger,
          ]
        : [
            activity.repositoryLabel,
            activity.event.changeRequest.title,
            activity.event.changeRequest.author,
            activity.event.changeRequest.sourceBranch,
            activity.event.changeRequest.targetBranch,
            activity.event.kind,
          ]
    )
      .filter(Boolean)
      .join(" ")
      .toLocaleLowerCase();
    return (
      searchable.includes(normalizedQuery) &&
      (filters.type === "all" ||
        (filters.type === "workflows" && activity.type === "workflow") ||
        (filters.type === "pullRequests" && activity.type === "pullRequest")) &&
      (activity.type === "workflow"
        ? matchesStatus(activity.run, filters.status)
        : matchesChangeRequestStatus(activity.event, filters.status)) &&
      (filters.repository.length === 0 || activity.repositoryId === filters.repository) &&
      (filters.trigger.length === 0 ||
        (activity.type === "workflow" && activity.run.trigger === filters.trigger)) &&
      (cutoff === null || Date.parse(activity.occurredAt) >= cutoff)
    );
  });
  const filtersActive =
    normalizedQuery.length > 0 ||
    filters.type !== "all" ||
    filters.status !== "all" ||
    filters.repository.length > 0 ||
    filters.trigger.length > 0 ||
    filters.range !== "all";

  const updateFilters = (values: ActivityFilterValues) => {
    const next = new URLSearchParams();
    if (values.query.trim().length > 0) next.set("q", values.query);
    if (values.type !== "all") next.set("type", values.type);
    if (values.status !== "all") next.set("status", values.status);
    if (values.repository.length > 0) next.set("repository", values.repository);
    if (values.trigger.length > 0) next.set("trigger", values.trigger);
    if (values.range !== "all") next.set("range", values.range);
    setSearchParams(next, { replace: true });
  };

  return (
    <section className="page-view" aria-labelledby="activity-title">
      <PageHeader
        eyebrow="Operations timeline"
        title="Activity"
        description="Workflow runs and pull request transitions across monitored repositories."
      />

      <section className="panel activity-panel" aria-labelledby="activity-list-title">
        <PanelHeader
          label="Timeline"
          title="Recent activity"
          metadata={
            <span className="panel-badge">
              {error
                ? "Unavailable"
                : filtersActive
                  ? `${filteredActivities.length} of ${activities.length} events`
                  : `${activities.length} event${activities.length === 1 ? "" : "s"}`}
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
                  type: "all",
                  status: "all",
                  repository: "",
                  trigger: "",
                  range: "all",
                })
              }
            />
            <ActivityListHeader />
          </>
        ) : null}
        <div className="activity-list" aria-live="polite">
          {loading ? (
            <p className="activity-loading">Loading cached activity…</p>
          ) : error ? (
            <EmptyState message="The activity timeline could not be loaded." error />
          ) : activities.length === 0 ? (
            <EmptyState message="No activity has been cached yet." />
          ) : filteredActivities.length === 0 ? (
            <EmptyState message="No activity matches the current filters." />
          ) : (
            filteredActivities.map((activity) =>
              activity.type === "workflow" ? (
                <ActivityRow
                  activity={activity}
                  key={activity.id}
                  onOpen={() => setSelected({ workflow: activity.workflow, run: activity.run })}
                />
              ) : (
                <ChangeRequestActivityRow
                  activity={activity.event}
                  key={activity.id}
                  onOpen={() => setSelectedChangeRequest(activity.event)}
                />
              ),
            )
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
      {selectedChangeRequest ? (
        <ChangeRequestDetailsDialog
          changeRequest={selectedChangeRequest.changeRequest}
          onClose={() => setSelectedChangeRequest(null)}
        />
      ) : null}
    </section>
  );
}
