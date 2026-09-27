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

export function formatRelativeDate(value: string): string {
  const timestamp = Date.parse(value);
  return Number.isNaN(timestamp) ? "Unknown time" : formatRelativeMilliseconds(timestamp);
}

export function formatRelativeUnix(value: number): string {
  return formatRelativeMilliseconds(value * 1000);
}

function formatRelativeMilliseconds(timestamp: number): string {
  const elapsedSeconds = Math.max(0, Math.floor((Date.now() - timestamp) / 1000));
  if (elapsedSeconds < 60) return "just now";
  const minutes = Math.floor(elapsedSeconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  return new Date(timestamp).toLocaleDateString();
}
