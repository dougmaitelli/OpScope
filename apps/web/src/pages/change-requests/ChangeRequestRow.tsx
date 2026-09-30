import {
  DataRow,
  DataRowIdentity,
  DataRowMeta,
  DataRowHeader,
  RowChevron,
} from "../../components/data-row/DataRow.tsx";
import type {
  ChangeRequestCheckStatus,
  ChangeRequestMergeStatus,
  ChangeRequestReviewStatus,
  ChangeRequestSummary,
} from "../../generated/contracts.ts";
import { formatRelativeDate } from "../../shared/workflow-runs.ts";
import "./ChangeRequestRow.css";
import { changeRequestStatusTone } from "../../shared/change-request-status.ts";
import { StatusPill } from "../../components/StatusPill.tsx";

const reviewLabels: Record<ChangeRequestReviewStatus, string> = {
  approved: "Approved",
  changesRequested: "Changes requested",
  reviewRequired: "Review required",
  unknown: "Review unknown",
};

const checkLabels: Record<ChangeRequestCheckStatus, string> = {
  passed: "Checks passed",
  failing: "Checks failing",
  running: "Checks running",
  unknown: "Checks unknown",
};

const mergeLabels: Record<ChangeRequestMergeStatus, string> = {
  ready: "Ready",
  blocked: "Blocked",
  conflicting: "Conflicts",
  unknown: "Unknown",
};

export function ChangeRequestRow({
  changeRequest,
  onOpen,
}: {
  changeRequest: ChangeRequestSummary;
  onOpen: () => void;
}) {
  return (
    <DataRow className="change-request-row" onClick={onOpen}>
      <DataRowIdentity
        className="change-request-primary"
        title={
          <>
            <span className="change-request-number">#{changeRequest.number}</span>
            {changeRequest.title}
            {changeRequest.draft ? (
              <StatusPill className="change-request-draft">Draft</StatusPill>
            ) : null}
          </>
        }
        metadata={
          <>
            {changeRequest.author ?? "Unknown author"} · {changeRequest.sourceBranch} →{" "}
            {changeRequest.targetBranch}
          </>
        }
      />
      <StatusPill
        className="change-request-state"
        tone={changeRequestStatusTone[changeRequest.reviewStatus]}
      >
        {reviewLabels[changeRequest.reviewStatus]}
      </StatusPill>
      <StatusPill
        className="change-request-state"
        tone={changeRequestStatusTone[changeRequest.checkStatus]}
      >
        {checkLabels[changeRequest.checkStatus]}
      </StatusPill>
      <StatusPill
        className="change-request-state"
        tone={changeRequestStatusTone[changeRequest.mergeStatus]}
      >
        {mergeLabels[changeRequest.mergeStatus]}
      </StatusPill>
      <DataRowMeta className="change-request-updated">
        {formatRelativeDate(changeRequest.updatedAt)}
      </DataRowMeta>
      <RowChevron className="change-request-open" />
    </DataRow>
  );
}

export function ChangeRequestListHeader() {
  return (
    <DataRowHeader className="change-request-list-header">
      <span>Pull request</span>
      <span>Review</span>
      <span>Checks</span>
      <span>Merge</span>
      <span>Updated</span>
      <span />
    </DataRowHeader>
  );
}
