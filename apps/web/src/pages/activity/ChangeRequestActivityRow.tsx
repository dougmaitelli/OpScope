import {
  DataRow,
  DataRowIdentity,
  DataRowMeta,
  RowChevron,
} from "../../components/data-row/DataRow.tsx";
import type {
  ChangeRequestActivityKind,
  ChangeRequestActivitySummary,
} from "../../generated/contracts.ts";
import { TimeDisplay } from "../../components/TimeDisplay.tsx";
import "./ActivityRow.css";
import { StatusPill } from "../../components/StatusPill.tsx";

const presentation: Record<
  ChangeRequestActivityKind,
  { label: string; tone: string; description: string }
> = {
  opened: { label: "Opened", tone: "queued", description: "Pull request opened" },
  readyForReview: { label: "Ready", tone: "queued", description: "Marked ready for review" },
  reviewApproved: { label: "Approved", tone: "success", description: "Review approved" },
  changesRequested: {
    label: "Changes",
    tone: "failure",
    description: "Changes requested",
  },
  checksFailed: { label: "Failed", tone: "failure", description: "Checks started failing" },
  checksRecovered: {
    label: "Recovered",
    tone: "success",
    description: "Checks recovered",
  },
  conflictDetected: {
    label: "Conflict",
    tone: "failure",
    description: "Merge conflict detected",
  },
  conflictResolved: {
    label: "Resolved",
    tone: "success",
    description: "Merge conflict resolved",
  },
  merged: { label: "Merged", tone: "success", description: "Pull request merged" },
  closed: { label: "Closed", tone: "cancelled", description: "Pull request closed" },
};

export function ChangeRequestActivityRow({
  activity,
  onOpen,
}: {
  activity: ChangeRequestActivitySummary;
  onOpen: () => void;
}) {
  const changeRequest = activity.changeRequest;
  const state = presentation[activity.kind];
  const repository = `${changeRequest.repositoryOwner}/${changeRequest.repositoryName}`;
  return (
    <DataRow
      className="activity-row"
      onClick={onOpen}
      aria-label={`Open pull request ${changeRequest.number} in ${repository}`}
    >
      <span className="activity-state-cell">
        <StatusPill tone={state.tone}>{state.label}</StatusPill>
        <DataRowMeta>
          <TimeDisplay dateTime={activity.occurredAt} />
        </DataRowMeta>
      </span>
      <DataRowIdentity
        className="activity-run-cell"
        title={
          <>
            #{changeRequest.number} · {changeRequest.title}
          </>
        }
        metadata={<>{state.description}</>}
      />
      <DataRowIdentity
        className="activity-project-cell"
        title={<>{repository}</>}
        metadata={<>Pull request · {changeRequest.sourceName}</>}
      />
      <DataRowIdentity
        className="activity-ref-cell"
        title={<>{changeRequest.sourceBranch}</>}
        metadata={<>→ {changeRequest.targetBranch}</>}
      />
      <DataRowIdentity
        className="activity-actor-cell"
        title={<>{changeRequest.author ?? "Unknown actor"}</>}
        metadata={<>pull request</>}
      />
      <RowChevron className="activity-row-chevron" />
    </DataRow>
  );
}
