import {
  DataRow,
  DataRowIdentity,
  DataRowMeta,
  RowChevron,
} from "../../components/data-row/DataRow.tsx";
import { StatusPill } from "../../components/StatusPill.tsx";
import { TimeDisplay } from "../../components/TimeDisplay.tsx";
import type { IssueActivityKind, IssueActivitySummary } from "../../generated/contracts.ts";
import "./ActivityRow.css";

const presentation: Record<IssueActivityKind, { label: string; tone: string }> = {
  opened: { label: "Opened", tone: "queued" },
  updated: { label: "Updated", tone: "neutral" },
  closed: { label: "Closed", tone: "success" },
  reopened: { label: "Reopened", tone: "queued" },
};

export function IssueActivityRow({
  activity,
  onOpen,
}: {
  activity: IssueActivitySummary;
  onOpen: () => void;
}) {
  const { issue } = activity;
  const state = presentation[activity.kind];
  const repository = `${issue.repositoryOwner}/${issue.repositoryName}`;
  return (
    <DataRow
      className="activity-row"
      onClick={onOpen}
      aria-label={`Open issue ${issue.number} in ${repository}`}
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
            #{issue.number} · {issue.title}
          </>
        }
        metadata={<>Issue {state.label.toLowerCase()}</>}
      />
      <DataRowIdentity
        className="activity-project-cell"
        title={<>{repository}</>}
        metadata={<>Issue · {issue.sourceName}</>}
      />
      <DataRowIdentity
        className="activity-ref-cell"
        title={<>{issue.assignees.join(", ") || "Unassigned"}</>}
        metadata={<>{issue.labels.join(", ") || "No labels"}</>}
      />
      <DataRowIdentity
        className="activity-actor-cell"
        title={<>{issue.author ?? "Unknown author"}</>}
        metadata={<>issue author</>}
      />
      <RowChevron className="activity-row-chevron" />
    </DataRow>
  );
}
