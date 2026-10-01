import { DataRow, DataRowGroup, DataRowIdentity } from "./data-row/DataRow.tsx";
import { useState } from "react";
import type { WorkflowRunSummary, WorkflowSummary } from "../generated/contracts.ts";
import "./WorkflowRow.css";
import { WorkflowRunLogsDialog } from "./WorkflowRunLogsDialog.tsx";
import { WorkflowRunState } from "./WorkflowRunState.tsx";
import { CommitLink } from "./CommitLink.tsx";

export function WorkflowRow({
  workflow,
  onRunUpdated,
}: {
  workflow: WorkflowSummary;
  onRunUpdated?: (workflow: WorkflowSummary, run: WorkflowRunSummary) => void;
}) {
  const [selectedRun, setSelectedRun] = useState<WorkflowRunSummary | null>(null);
  const latestRun = workflow.runs[0] ?? null;
  const earlierRuns = workflow.runs.slice(1);

  return (
    <DataRowGroup className="workflow-row">
      <DataRow
        as="div"
        surface={false}
        className={`workflow-row-summary${latestRun ? " workflow-row-summary-clickable" : ""}`}
        onClick={() => {
          if (latestRun) setSelectedRun(latestRun);
        }}
      >
        <DataRowIdentity
          className="workflow-identity"
          titleAs="h4"
          title={
            <a
              className="workflow-link"
              href={workflow.webUrl}
              target="_blank"
              rel="noreferrer"
              onClick={(event) => event.stopPropagation()}
            >
              {workflow.name}
            </a>
          }
          metadataClassName={latestRun ? "latest-run" : ""}
          metadata={
            latestRun ? (
              <>
                <button
                  className="run-link-button"
                  type="button"
                  onClick={() => setSelectedRun(latestRun)}
                >
                  #{latestRun.runNumber} · {latestRun.title}
                </button>
                <span>
                  {latestRun.branch ?? "detached"} ·{" "}
                  <CommitLink sha={latestRun.commitSha} resourceUrl={latestRun.webUrl} />
                  {latestRun.actor ? ` · ${latestRun.actor}` : ""}
                </span>
              </>
            ) : (
              workflow.path
            )
          }
        />
        <WorkflowRunState run={latestRun} workflowState={workflow.state} />
      </DataRow>
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
                  <span>
                    {run.branch ?? "detached"} ·{" "}
                    <CommitLink sha={run.commitSha} resourceUrl={run.webUrl} />
                  </span>
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
          onUpdated={(run) => onRunUpdated?.(workflow, run)}
        />
      ) : null}
    </DataRowGroup>
  );
}
