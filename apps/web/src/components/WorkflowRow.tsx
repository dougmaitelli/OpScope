import type { WorkflowSummary } from "../generated/contracts.ts";

export function WorkflowRow({ workflow }: { workflow: WorkflowSummary }) {
  return (
    <article className="workflow-row">
      <div className="workflow-identity">
        <span className="workflow-mark" aria-hidden="true">
          {workflow.name.slice(0, 2).toUpperCase()}
        </span>
        <div>
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
          <p>{workflow.path}</p>
        </div>
      </div>
      <span className={`workflow-state workflow-state-${workflow.state}`}>
        {workflow.state === "active" ? "Active" : "Disabled"}
      </span>
    </article>
  );
}
