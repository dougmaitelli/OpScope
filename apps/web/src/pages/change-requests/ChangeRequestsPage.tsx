import { useFeatureRefresh } from "../../api/use-feature-refresh.ts";
import { RefreshButton } from "../../components/RefreshButton.tsx";
import { useSynchronizedData } from "../../api/use-synchronized-data.ts";
import { useMemo, useState } from "react";
import { useApplicationClient } from "../../api/application-client.tsx";
import { EmptyState } from "../../components/EmptyState.tsx";
import { PageHeader } from "../../components/PageHeader.tsx";
import { PanelHeader } from "../../components/PanelHeader.tsx";
import { InventorySummary } from "../../components/InventorySummary.tsx";
import type {
  ChangeRequestSummary,
  ListChangeRequestsResponse,
} from "../../generated/contracts.ts";
import { ChangeRequestFilters, type ChangeRequestFilter } from "./ChangeRequestFilters.tsx";
import { ChangeRequestDetailsDialog } from "./ChangeRequestDetailsDialog.tsx";
import { ChangeRequestProjectGroup } from "./ChangeRequestProjectGroup.tsx";
import { ProjectGroupList } from "../../components/ProjectGroup.tsx";
import { groupByProject } from "../../shared/project-groups.ts";
import "./ChangeRequestsPage.css";

function matchesStatus(item: ChangeRequestSummary, status: ChangeRequestFilter): boolean {
  return (
    status === "all" ||
    (status === "draft" && item.draft) ||
    (status === "ready" &&
      !item.draft &&
      item.reviewStatus === "approved" &&
      item.checkStatus === "passed" &&
      item.mergeStatus === "ready") ||
    (status === "attention" &&
      (item.reviewStatus === "changesRequested" ||
        item.checkStatus === "failing" ||
        item.mergeStatus === "conflicting"))
  );
}

export function ChangeRequestsPage() {
  const client = useApplicationClient();
  const {
    data: inventory,
    setData: setInventory,
    loading,
    error,
    setError,
  } = useSynchronizedData<ListChangeRequestsResponse>(() => client.listChangeRequests());
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState<ChangeRequestFilter>("all");
  const [selected, setSelected] = useState<ChangeRequestSummary | null>(null);

  const { refresh, refreshing, notice } = useFeatureRefresh("pullRequests", async () => {
    setInventory(await client.listChangeRequests());
    setError(null);
  });

  const changeRequests = useMemo(() => inventory?.changeRequests ?? [], [inventory]);
  const filtered = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    return changeRequests.filter((item) => {
      const searchable = [
        item.title,
        item.author,
        item.repositoryOwner,
        item.repositoryName,
        item.sourceBranch,
        item.targetBranch,
      ]
        .filter(Boolean)
        .join(" ")
        .toLocaleLowerCase();
      return searchable.includes(normalized) && matchesStatus(item, status);
    });
  }, [changeRequests, query, status]);

  const projects = useMemo(() => groupByProject(filtered), [filtered]);
  const attention = changeRequests.filter((item) => matchesStatus(item, "attention")).length;
  const unavailable = !!error || !inventory;
  const totals: { label: string; status: ChangeRequestFilter; count: number }[] = [
    { label: "Open PRs", status: "all", count: changeRequests.length },
    {
      label: "Ready",
      status: "ready",
      count: changeRequests.filter((item) => matchesStatus(item, "ready")).length,
    },
    { label: "Draft", status: "draft", count: changeRequests.filter((item) => item.draft).length },
    { label: "Needs attention", status: "attention", count: attention },
  ];

  return (
    <section className="page-view" aria-labelledby="change-requests-title">
      <PageHeader
        eyebrow="Code changes"
        title="Pull requests"
        description="Review readiness, checks, and merge state across monitored repositories."
        actions={<RefreshButton busy={refreshing} disabled={loading} onRefresh={refresh} />}
      />
      <InventorySummary
        heading={
          error
            ? "Unable to load pull requests"
            : loading && !inventory
              ? "Loading pull requests"
              : attention > 0
                ? `${attention} pull request${attention === 1 ? " needs" : "s need"} attention`
                : `${changeRequests.length} open pull request${changeRequests.length === 1 ? "" : "s"}`
        }
        description={
          error
            ? "Check that the application service is running"
            : inventory
              ? `${inventory.selectedRepositoryCount} selected repositor${inventory.selectedRepositoryCount === 1 ? "y" : "ies"}`
              : "Waiting for the application core"
        }
        tone={error || attention > 0 ? "failing" : "idle"}
        busy={loading || refreshing}
        stats={totals.map((total) => ({
          label: total.label,
          value: unavailable ? "—" : total.count,
          tone: total.status === "attention" ? "failing" : "idle",
          actionLabel:
            total.status === "all"
              ? "Show all open pull requests"
              : total.status === "attention"
                ? "Show pull requests needing attention"
                : `Show ${total.label.toLocaleLowerCase()} pull requests`,
          onSelect:
            !unavailable && total.count > 0
              ? () => {
                  setQuery("");
                  setStatus(total.status);
                }
              : undefined,
        }))}
      />
      {notice ? <p role="status">{notice}</p> : null}
      <section className="panel change-requests-panel" aria-labelledby="change-request-list-title">
        <PanelHeader
          label="Open work"
          title="Pull requests"
          metadata={
            <span className="panel-badge">
              {error ? "Unavailable" : `${filtered.length} of ${changeRequests.length}`}
            </span>
          }
        />
        {changeRequests.length > 0 ? (
          <>
            <ChangeRequestFilters
              query={query}
              status={status}
              onQueryChange={setQuery}
              onStatusChange={setStatus}
            />
          </>
        ) : null}
        <ProjectGroupList className="change-request-list" aria-live="polite">
          {loading ? (
            <p className="change-request-loading">Loading cached pull requests…</p>
          ) : error ? (
            <EmptyState message={error} error />
          ) : (inventory?.selectedRepositoryCount ?? 0) === 0 ? (
            <EmptyState message="Select repositories to monitor before loading pull requests." />
          ) : changeRequests.length === 0 ? (
            <EmptyState message="No open pull requests were found in the monitored repositories." />
          ) : filtered.length === 0 ? (
            <EmptyState message="No pull requests match the current filters." />
          ) : (
            projects.map((project) => (
              <ChangeRequestProjectGroup key={project.id} project={project} onOpen={setSelected} />
            ))
          )}
        </ProjectGroupList>
      </section>
      {selected ? (
        <ChangeRequestDetailsDialog
          changeRequest={selected}
          onClose={() => setSelected(null)}
          onUpdated={(updated) => {
            setSelected(updated);
            setInventory(
              (current) =>
                current && {
                  ...current,
                  changeRequests: current.changeRequests.flatMap((item) =>
                    item.sourceId === updated.sourceId &&
                    item.repositoryId === updated.repositoryId &&
                    item.number === updated.number
                      ? updated.state === "open"
                        ? [updated]
                        : []
                      : [item],
                  ),
                },
            );
          }}
        />
      ) : null}
    </section>
  );
}
