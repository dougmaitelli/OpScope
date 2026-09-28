import type { WorkflowRunSummary, WorkflowState } from "../generated/contracts.ts";
import { formatRelativeDate, formatRunDuration, runPresentation } from "../shared/workflow-runs.ts";
import "./WorkflowRunState.css";
import { StatusPill } from "./StatusPill.tsx";

interface WorkflowRunStateProps {
  run: WorkflowRunSummary | null;
  workflowState?: WorkflowState;
}

export function WorkflowRunState({ run, workflowState }: WorkflowRunStateProps) {
  if (!run) {
    const disabled = workflowState === "disabled";
    return (
      <div className="run-state-block">
        <StatusPill>{disabled ? "Disabled" : "No runs"}</StatusPill>
      </div>
    );
  }

  const presentation = runPresentation(run);
  const duration = formatRunDuration(run);
  return (
    <div className="run-state-block">
      <StatusPill tone={presentation.tone}>{presentation.label}</StatusPill>
      <span className="run-timing">
        {duration ? `${duration}${run.lifecycle === "running" ? " elapsed" : ""} · ` : ""}
        <time dateTime={run.updatedAt}>{formatRelativeDate(run.updatedAt)}</time>
      </span>
    </div>
  );
}
