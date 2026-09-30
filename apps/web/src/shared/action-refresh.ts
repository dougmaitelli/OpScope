import type { WorkflowRunSummary, WorkflowSummary } from "../generated/contracts.ts";

/** Patch one run in a loaded inventory; no collection request is needed. */
export function updateWorkflowRun(
  workflows: WorkflowSummary[],
  target: WorkflowSummary,
  run: WorkflowRunSummary,
): WorkflowSummary[] {
  return workflows.map((workflow) => {
    if (
      workflow.sourceId !== target.sourceId ||
      workflow.repositoryId !== target.repositoryId ||
      workflow.id !== target.id
    )
      return workflow;
    const runs = [...workflow.runs.filter((existing) => existing.id !== run.id), run]
      .sort(
        (a, b) =>
          b.runNumber - a.runNumber ||
          b.attempt - a.attempt ||
          b.createdAt.localeCompare(a.createdAt),
      )
      .slice(0, Math.max(1, workflow.runs.length));
    return { ...workflow, runs };
  });
}
