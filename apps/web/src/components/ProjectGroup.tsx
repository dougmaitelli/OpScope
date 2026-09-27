import type { WorkflowSummary } from "../generated/contracts.ts";
import { WorkflowRow } from "./WorkflowRow.tsx";
import "./ProjectGroup.css";

export interface WorkflowProject {
  id: string;
  sourceName: string;
  sourceAbbreviation: string;
  repositoryOwner: string;
  repositoryName: string;
  workflows: WorkflowSummary[];
}

export function groupWorkflows(workflows: WorkflowSummary[]): WorkflowProject[] {
  const projects = new Map<string, WorkflowProject>();
  for (const workflow of workflows) {
    const id = `${workflow.sourceId}:${workflow.repositoryId}`;
    const project = projects.get(id);
    if (project) {
      project.workflows.push(workflow);
    } else {
      projects.set(id, {
        id,
        sourceName: workflow.sourceName,
        sourceAbbreviation: workflow.sourceAbbreviation,
        repositoryOwner: workflow.repositoryOwner,
        repositoryName: workflow.repositoryName,
        workflows: [workflow],
      });
    }
  }
  return [...projects.values()].sort((left, right) =>
    `${left.repositoryOwner}/${left.repositoryName}`.localeCompare(
      `${right.repositoryOwner}/${right.repositoryName}`,
      undefined,
      { sensitivity: "base" },
    ),
  );
}

export function ProjectGroup({ project }: { project: WorkflowProject }) {
  const active = project.workflows.filter((workflow) => workflow.state === "active").length;
  return (
    <details className="project-group" open>
      <summary className="project-header">
        <div className="project-identity">
          <span className="project-mark" aria-hidden="true">
            {project.sourceAbbreviation}
          </span>
          <div>
            <h3>
              {project.repositoryOwner}/{project.repositoryName}
            </h3>
            <p>
              {project.sourceName} · {project.workflows.length} workflow
              {project.workflows.length === 1 ? "" : "s"}
            </p>
          </div>
        </div>
        <div className="project-metadata">
          <span>{active} active</span>
          <span className="project-chevron" aria-hidden="true">
            <svg viewBox="0 0 16 16">
              <path d="m4.5 6 3.5 3.5L11.5 6" />
            </svg>
          </span>
        </div>
      </summary>
      <div className="project-workflows">
        {project.workflows.map((workflow) => (
          <WorkflowRow key={workflow.id} workflow={workflow} />
        ))}
      </div>
    </details>
  );
}
