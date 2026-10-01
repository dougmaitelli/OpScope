import {
  DataRow,
  DataRowIdentity,
  DataRowMeta,
  DataRowHeader,
  RowChevron,
} from "../../components/data-row/DataRow.tsx";
import type { IssueSummary } from "../../generated/contracts.ts";
import { TimeDisplay } from "../../components/TimeDisplay.tsx";
import "./IssueRow.css";

export function IssueRow({ issue, onOpen }: { issue: IssueSummary; onOpen: () => void }) {
  return (
    <DataRow className="issue-row" onClick={onOpen}>
      <DataRowIdentity
        className="issue-primary"
        title={
          <>
            <span className="issue-number">#{issue.number}</span>
            {issue.title}
          </>
        }
        metadata={<>Opened by {issue.author ?? "Unknown author"}</>}
      />
      <DataRowMeta className="issue-assignees">
        {issue.assignees.length > 0 ? issue.assignees.join(", ") : "Unassigned"}
      </DataRowMeta>
      <span className="issue-labels">
        {issue.labels.length > 0 ? (
          <>
            {issue.labels.slice(0, 2).map((label) => (
              <span key={label}>{label}</span>
            ))}
            {issue.labels.length > 2 ? <small>+{issue.labels.length - 2}</small> : null}
          </>
        ) : (
          <small>None</small>
        )}
      </span>
      <DataRowMeta className="issue-comments">{issue.commentCount}</DataRowMeta>
      <DataRowMeta className="issue-updated">
        <TimeDisplay dateTime={issue.updatedAt} />
      </DataRowMeta>
      <RowChevron className="issue-open" />
    </DataRow>
  );
}

export function IssueListHeader() {
  return (
    <DataRowHeader className="issue-list-header">
      <span>Issue</span>
      <span>Assignees</span>
      <span>Labels</span>
      <span>Comments</span>
      <span>Updated</span>
      <span />
    </DataRowHeader>
  );
}
