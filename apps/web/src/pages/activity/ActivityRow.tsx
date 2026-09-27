import type {
  WorkflowRunSummary,
  WorkflowSummary,
} from "../../generated/contracts.ts";
import { formatRelativeDate, runPresentation } from "../../shared/workflow-runs.ts";
import "./ActivityRow.css";

export interface WorkflowActivity {
  id: string;
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
  return (
    <button
      className="activity-row"
      type="button"
      onClick={onOpen}
      aria-label={`Open logs for ${activity.repositoryLabel}, ${workflow.name}, run ${run.runNumber}`}
    >
      <span className="activity-state-cell">
        <span className={`activity-state activity-state-${presentation.tone}`}>
          {presentation.label}
        </span>
        <time dateTime={run.createdAt}>{formatRelativeDate(run.createdAt)}</time>
      </span>
      <span className="activity-run-cell">
        <strong>#{run.runNumber} · {run.title}</strong>
        <span>
          {run.attempt > 1 ? `Attempt ${run.attempt} · ` : ""}{workflow.sourceName}
        </span>
      </span>
      <span className="activity-project-cell">
        <strong>{activity.repositoryLabel}</strong>
        <span>{workflow.name}</span>
      </span>
      <span className="activity-ref-cell">
        <strong>{run.branch ?? "detached"}</strong>
        <span>{run.commitSha.slice(0, 7)}</span>
      </span>
      <span className="activity-actor-cell">
        <strong>{run.actor ?? "Unknown actor"}</strong>
        <span>{run.trigger}</span>
      </span>
      <span className="activity-row-chevron" aria-hidden="true">›</span>
    </button>
  );
}
