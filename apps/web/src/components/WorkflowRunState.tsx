import type { WorkflowRunSummary, WorkflowState } from "../generated/contracts.ts";
import { formatRunDuration, runPresentation } from "../shared/workflow-runs.ts";
import { TimeDisplay } from "./TimeDisplay.tsx";
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
        <TimeDisplay
          dateTime={run.updatedAt}
          duration={duration}
          elapsed={run.lifecycle === "running"}
        />
      </span>
    </div>
  );
}
