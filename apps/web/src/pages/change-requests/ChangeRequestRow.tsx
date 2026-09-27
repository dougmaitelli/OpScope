import type {
  ChangeRequestCheckStatus,
  ChangeRequestMergeStatus,
  ChangeRequestReviewStatus,
  ChangeRequestSummary,
} from "../../generated/contracts.ts";
import { formatRelativeDate } from "../../shared/workflow-runs.ts";
import "./ChangeRequestRow.css";

const reviewLabels: Record<ChangeRequestReviewStatus, string> = {
  approved: "Approved",
  changesRequested: "Changes requested",
  reviewRequired: "Review required",
  unknown: "No decision",
};

const checkLabels: Record<ChangeRequestCheckStatus, string> = {
  passed: "Checks passed",
  failing: "Checks failing",
  running: "Checks running",
  unknown: "No checks",
};

const mergeLabels: Record<ChangeRequestMergeStatus, string> = {
  ready: "Ready",
  blocked: "Blocked",
  conflicting: "Conflicts",
  unknown: "Unknown",
};

export function ChangeRequestRow({ changeRequest }: { changeRequest: ChangeRequestSummary }) {
  const open = () => window.open(changeRequest.webUrl, "_blank", "noopener,noreferrer");
  return (
    <button className="change-request-row" type="button" onClick={open}>
      <span className="change-request-primary">
        <span className="change-request-title">
          <span className="change-request-number">#{changeRequest.number}</span>
          {changeRequest.title}
          {changeRequest.draft ? <span className="change-request-draft">Draft</span> : null}
        </span>
        <span className="change-request-meta">
          {changeRequest.author ?? "Unknown author"} · {changeRequest.sourceBranch} →{" "}
          {changeRequest.targetBranch}
        </span>
      </span>
      <span className="change-request-repository">
        <span>
          {changeRequest.repositoryOwner}/{changeRequest.repositoryName}
        </span>
        <small>{changeRequest.sourceName}</small>
      </span>
      <span className={`change-request-state state-review-${changeRequest.reviewStatus}`}>
        {reviewLabels[changeRequest.reviewStatus]}
      </span>
      <span className={`change-request-state state-check-${changeRequest.checkStatus}`}>
        {checkLabels[changeRequest.checkStatus]}
      </span>
      <span className={`change-request-state state-merge-${changeRequest.mergeStatus}`}>
        {mergeLabels[changeRequest.mergeStatus]}
      </span>
      <span className="change-request-updated">{formatRelativeDate(changeRequest.updatedAt)}</span>
      <span className="change-request-open" aria-hidden="true">
        ↗
      </span>
    </button>
  );
}
