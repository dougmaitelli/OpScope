import type { WorkflowRunSummary, WorkflowState } from "../generated/contracts.ts";
import { formatRelativeDate, runPresentation } from "../shared/workflow-runs.ts";
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
        <span className="run-state run-state-idle">
          {disabled ? "Disabled" : "No runs"}
        </span>
      </div>
    );
  }

  const presentation = runPresentation(run);
  return (
    <div className="run-state-block">
      <span className={`run-state run-state-${presentation.tone}`}>
        {presentation.label}
      </span>
      <time dateTime={run.updatedAt}>{formatRelativeDate(run.updatedAt)}</time>
    </div>
  );
}
