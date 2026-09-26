import type { WorkflowSummary } from "../generated/contracts.ts";
import "./WorkflowRow.css";
import { WorkflowRunState } from "./WorkflowRunState.tsx";

export function WorkflowRow({ workflow }: { workflow: WorkflowSummary }) {
  const latestRun = workflow.runs[0] ?? null;
  const earlierRuns = workflow.runs.slice(1);

  return (
    <article className="workflow-row">
      <div className="workflow-row-summary">
        <div className="workflow-identity">
          <span className="workflow-mark" aria-hidden="true">
            {workflow.name.slice(0, 2).toUpperCase()}
          </span>
          <div className="workflow-copy">
            <h4>
              <a
                className="workflow-link"
                href={workflow.webUrl}
                target="_blank"
                rel="noreferrer"
              >
                {workflow.name}
              </a>
            </h4>
            {latestRun ? (
              <p className="latest-run">
                <a href={latestRun.webUrl} target="_blank" rel="noreferrer">
                  #{latestRun.runNumber} · {latestRun.title}
                </a>
                <span>
                  {latestRun.branch ?? "detached"} · {latestRun.commitSha.slice(0, 7)}
                  {latestRun.actor ? ` · ${latestRun.actor}` : ""}
                </span>
              </p>
            ) : (
              <p>{workflow.path}</p>
            )}
          </div>
        </div>
        <WorkflowRunState run={latestRun} workflowState={workflow.state} />
      </div>
      {earlierRuns.length > 0 ? (
        <details className="run-history">
          <summary>Previous runs</summary>
          <div className="run-history-list">
            {earlierRuns.map((run) => (
              <div className="run-history-row" key={`${run.id}:${run.attempt}`}>
                <div>
                  <a href={run.webUrl} target="_blank" rel="noreferrer">
                    #{run.runNumber} · {run.title}
                  </a>
                  <span>{run.branch ?? "detached"} · {run.commitSha.slice(0, 7)}</span>
                </div>
                <WorkflowRunState run={run} />
              </div>
            ))}
          </div>
        </details>
      ) : null}
    </article>
  );
}
