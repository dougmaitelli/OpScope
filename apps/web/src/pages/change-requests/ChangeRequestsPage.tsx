import { useEffect, useMemo, useState } from "react";
import { useApplicationClient } from "../../api/application-client.tsx";
import { EmptyState } from "../../components/EmptyState.tsx";
import { PageHeader } from "../../components/PageHeader.tsx";
import { PanelHeader } from "../../components/PanelHeader.tsx";
import type {
  ChangeRequestSummary,
  ListChangeRequestsResponse,
} from "../../generated/contracts.ts";
import { requestErrorMessage } from "../../shared/errors.ts";
import { ChangeRequestFilters, type ChangeRequestFilter } from "./ChangeRequestFilters.tsx";
import { ChangeRequestDetailsDialog } from "./ChangeRequestDetailsDialog.tsx";
import { ChangeRequestRow } from "./ChangeRequestRow.tsx";
import "./ChangeRequestsPage.css";

export function ChangeRequestsPage() {
  const client = useApplicationClient();
  const [inventory, setInventory] = useState<ListChangeRequestsResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState<ChangeRequestFilter>("all");
  const [selected, setSelected] = useState<ChangeRequestSummary | null>(null);

  useEffect(() => {
    let active = true;
    void client
      .listChangeRequests()
      .then((response) => {
        if (active) setInventory(response);
      })
      .catch((failure: unknown) => {
        if (active) setError(requestErrorMessage(failure));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [client]);

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
      const matchesStatus =
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
            item.mergeStatus === "conflicting"));
      return searchable.includes(normalized) && matchesStatus;
    });
  }, [changeRequests, query, status]);

  return (
    <section className="page-view" aria-labelledby="change-requests-title">
      <PageHeader
        eyebrow="Code changes"
        title="Pull requests"
        description="Review readiness, checks, and merge state across monitored repositories."
      />
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
            <div className="change-request-list-header" aria-hidden="true">
              <span>Pull request</span>
              <span>Repository</span>
              <span>Review</span>
              <span>Checks</span>
              <span>Merge</span>
              <span>Updated</span>
              <span />
            </div>
          </>
        ) : null}
        <div className="change-request-list" aria-live="polite">
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
            filtered.map((item) => (
              <ChangeRequestRow
                key={`${item.sourceId}:${item.repositoryId}:${item.id}`}
                changeRequest={item}
                onOpen={() => setSelected(item)}
              />
            ))
          )}
        </div>
      </section>
      {selected ? (
        <ChangeRequestDetailsDialog changeRequest={selected} onClose={() => setSelected(null)} />
      ) : null}
    </section>
  );
}
