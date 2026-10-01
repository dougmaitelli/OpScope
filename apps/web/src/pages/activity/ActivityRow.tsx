import {
  DataRow,
  DataRowIdentity,
  DataRowMeta,
  DataRowHeader,
  RowChevron,
} from "../../components/data-row/DataRow.tsx";
import type { WorkflowRunSummary, WorkflowSummary } from "../../generated/contracts.ts";
import { formatRunDuration, runPresentation } from "../../shared/workflow-runs.ts";
import "./ActivityRow.css";
import { StatusPill } from "../../components/StatusPill.tsx";
import { TimeDisplay } from "../../components/TimeDisplay.tsx";

export interface WorkflowActivity {
  type: "workflow";
  id: string;
  occurredAt: string;
  repositoryId: string;
  repositoryLabel: string;
  workflow: WorkflowSummary;
  run: WorkflowRunSummary;
}

export function ActivityRow({
  activity,
  onOpen,
}: {
  activity: WorkflowActivity;
  onOpen: () => void;
}) {
  const { workflow, run } = activity;
  const presentation = runPresentation(run);
  const duration = formatRunDuration(run);
  return (
    <DataRow
      className="activity-row"
      onClick={onOpen}
      aria-label={`Open logs for ${activity.repositoryLabel}, ${workflow.name}, run ${run.runNumber}`}
    >
      <span className="activity-state-cell">
        <StatusPill tone={presentation.tone}>{presentation.label}</StatusPill>
        <DataRowMeta>
          <TimeDisplay
            dateTime={run.createdAt}
            duration={duration}
            elapsed={run.lifecycle === "running"}
          />
        </DataRowMeta>
      </span>
      <DataRowIdentity
        className="activity-run-cell"
        title={
          <>
            #{run.runNumber} · {run.title}
          </>
        }
        metadata={
          <>
            {run.attempt > 1 ? `Attempt ${run.attempt} · ` : ""}
            {workflow.sourceName}
          </>
        }
      />
      <DataRowIdentity
        className="activity-project-cell"
        title={<>{activity.repositoryLabel}</>}
        metadata={<>{workflow.name}</>}
      />
      <DataRowIdentity
        className="activity-ref-cell"
        title={<>{run.branch ?? "detached"}</>}
        metadata={<>{run.commitSha.slice(0, 7)}</>}
      />
      <DataRowIdentity
        className="activity-actor-cell"
        title={<>{run.actor ?? "Unknown actor"}</>}
        metadata={<>{run.trigger}</>}
      />
      <RowChevron className="activity-row-chevron" />
    </DataRow>
  );
}

export function ActivityListHeader() {
  return (
    <DataRowHeader className="activity-list-header">
      <span>Status</span>
      <span>Activity</span>
      <span>Project / context</span>
      <span className="activity-ref-heading">Ref</span>
      <span className="activity-actor-heading">Actor / trigger</span>
      <span />
    </DataRowHeader>
  );
}
