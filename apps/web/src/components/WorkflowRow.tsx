import { useState } from "react";
import type { WorkflowRunSummary, WorkflowSummary } from "../generated/contracts.ts";
import "./WorkflowRow.css";
import { WorkflowRunLogsDialog } from "./WorkflowRunLogsDialog.tsx";
import { WorkflowRunState } from "./WorkflowRunState.tsx";

export function WorkflowRow({ workflow }: { workflow: WorkflowSummary }) {
  const [selectedRun, setSelectedRun] = useState<WorkflowRunSummary | null>(null);
  const latestRun = workflow.runs[0] ?? null;
  const earlierRuns = workflow.runs.slice(1);

  return (
    <article className="workflow-row">
      <div
        className={`workflow-row-summary${latestRun ? " workflow-row-summary-clickable" : ""}`}
        onClick={() => {
          if (latestRun) setSelectedRun(latestRun);
        }}
      >
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
                onClick={(event) => event.stopPropagation()}
              >
                {workflow.name}
              </a>
            </h4>
            {latestRun ? (
              <p className="latest-run">
                <button
                  className="run-link-button"
                  type="button"
                  onClick={() => setSelectedRun(latestRun)}
                >
                  #{latestRun.runNumber} · {latestRun.title}
                </button>
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
              <div
                className="run-history-row"
                key={`${run.id}:${run.attempt}`}
                onClick={() => setSelectedRun(run)}
              >
                <div>
                  <button
                    className="run-link-button"
                    type="button"
                    onClick={() => setSelectedRun(run)}
                  >
                    #{run.runNumber} · {run.title}
                  </button>
                  <span>{run.branch ?? "detached"} · {run.commitSha.slice(0, 7)}</span>
                </div>
                <WorkflowRunState run={run} />
              </div>
            ))}
          </div>
        </details>
      ) : null}
      {selectedRun ? (
        <WorkflowRunLogsDialog
          workflow={workflow}
          run={selectedRun}
          onClose={() => setSelectedRun(null)}
        />
      ) : null}
    </article>
  );
}
