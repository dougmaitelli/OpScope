import type { RunLifecycle, RunOutcome, WorkflowRunSummary } from "../generated/contracts.ts";

export type RunTone =
  | "cancelled"
  | "failure"
  | "idle"
  | "queued"
  | "running"
  | "skipped"
  | "success"
  | "unknown"
  | "warning";

const outcomeLabels: Record<RunOutcome, string> = {
  success: "Passed",
  warning: "Warning",
  failure: "Failed",
  cancelled: "Cancelled",
  skipped: "Skipped",
  unknown: "Completed",
};

const lifecycleLabels: Record<RunLifecycle, string> = {
  queued: "Queued",
  running: "Running",
  completed: "Completed",
  unknown: "Unknown",
};

export function runPresentation(run: WorkflowRunSummary): {
  label: string;
  tone: RunTone;
} {
  if (run.lifecycle === "completed") {
    return { label: outcomeLabels[run.outcome], tone: run.outcome };
  }
  return {
    label: lifecycleLabels[run.lifecycle],
    tone: run.lifecycle === "unknown" ? "unknown" : run.lifecycle,
  };
}

export function formatRunDuration(
  run: WorkflowRunSummary,
  currentTime = Date.now(),
): string | null {
  if (!run.startedAt || (run.lifecycle !== "completed" && run.lifecycle !== "running")) {
    return null;
  }
  const startedAt = Date.parse(run.startedAt);
  const finishedAt = run.lifecycle === "completed" ? Date.parse(run.updatedAt) : currentTime;
  if (Number.isNaN(startedAt) || Number.isNaN(finishedAt) || finishedAt < startedAt) return null;

  const totalSeconds = Math.max(1, Math.floor((finishedAt - startedAt) / 1000));
  if (totalSeconds < 60) return `${totalSeconds}s`;
  const totalMinutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (totalMinutes < 60) return seconds > 0 ? `${totalMinutes}m ${seconds}s` : `${totalMinutes}m`;
  const totalHours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  if (totalHours < 24) return minutes > 0 ? `${totalHours}h ${minutes}m` : `${totalHours}h`;
  const days = Math.floor(totalHours / 24);
  const hours = totalHours % 24;
  return hours > 0 ? `${days}d ${hours}h` : `${days}d`;
}
