import type { IssueSummary } from "../../generated/contracts.ts";
import { formatRelativeDate } from "../../shared/workflow-runs.ts";
import "./IssueRow.css";

export function IssueRow({ issue, onOpen }: { issue: IssueSummary; onOpen: () => void }) {
  return (
    <button className="issue-row" type="button" onClick={onOpen}>
      <span className="issue-primary">
        <span className="issue-title">
          <span className="issue-number">#{issue.number}</span>
          {issue.title}
        </span>
        <span className="issue-meta">Opened by {issue.author ?? "Unknown author"}</span>
      </span>
      <span className="issue-repository">
        <span>
          {issue.repositoryOwner}/{issue.repositoryName}
        </span>
        <small>{issue.sourceName}</small>
      </span>
      <span className="issue-assignees">
        {issue.assignees.length > 0 ? issue.assignees.join(", ") : "Unassigned"}
      </span>
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
      <span className="issue-comments">{issue.commentCount}</span>
      <span className="issue-updated">{formatRelativeDate(issue.updatedAt)}</span>
      <span className="issue-open" aria-hidden="true">
        ↗
      </span>
    </button>
  );
}
