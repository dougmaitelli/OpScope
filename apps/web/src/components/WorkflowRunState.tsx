import type { WorkflowRunSummary, WorkflowState } from "../generated/contracts.ts";
import { formatRelativeDate, formatRunDuration, runPresentation } from "../shared/workflow-runs.ts";
import "./WorkflowRunState.css";

interface WorkflowRunStateProps {
  run: WorkflowRunSummary | null;
  workflowState?: WorkflowState;
}

export function WorkflowRunState({ run, workflowState }: WorkflowRunStateProps) {
  if (!run) {
    const disabled = workflowState === "disabled";
    return (
      <div className="run-state-block">
        <span className="run-state run-state-idle">{disabled ? "Disabled" : "No runs"}</span>
      </div>
    );
  }

  const presentation = runPresentation(run);
  const duration = formatRunDuration(run);
  return (
    <div className="run-state-block">
      <span className={`run-state run-state-${presentation.tone}`}>{presentation.label}</span>
      <span className="run-timing">
        {duration ? `${duration}${run.lifecycle === "running" ? " elapsed" : ""} · ` : ""}
        <time dateTime={run.updatedAt}>{formatRelativeDate(run.updatedAt)}</time>
      </span>
    </div>
  );
}
