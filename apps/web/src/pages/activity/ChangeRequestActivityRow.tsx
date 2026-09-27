import type {
  ChangeRequestActivityKind,
  ChangeRequestActivitySummary,
} from "../../generated/contracts.ts";
import { formatRelativeDate } from "../../shared/workflow-runs.ts";
import "./ActivityRow.css";

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
    <button
      className="activity-row"
      type="button"
      onClick={onOpen}
      aria-label={`Open pull request ${changeRequest.number} in ${repository}`}
    >
      <span className="activity-state-cell">
        <span className={`activity-state activity-state-${state.tone}`}>{state.label}</span>
        <time dateTime={activity.occurredAt}>{formatRelativeDate(activity.occurredAt)}</time>
      </span>
      <span className="activity-run-cell">
        <strong>
          #{changeRequest.number} · {changeRequest.title}
        </strong>
        <span>{state.description}</span>
      </span>
      <span className="activity-project-cell">
        <strong>{repository}</strong>
        <span>Pull request · {changeRequest.sourceName}</span>
      </span>
      <span className="activity-ref-cell">
        <strong>{changeRequest.sourceBranch}</strong>
        <span>→ {changeRequest.targetBranch}</span>
      </span>
      <span className="activity-actor-cell">
        <strong>{changeRequest.author ?? "Unknown actor"}</strong>
        <span>pull request</span>
      </span>
      <span className="activity-row-chevron" aria-hidden="true">
        ›
      </span>
    </button>
  );
}
