import { ProjectGroup } from "../../components/ProjectGroup.tsx";
import { WorkflowRow } from "../../components/WorkflowRow.tsx";
import type { WorkflowRunSummary, WorkflowSummary } from "../../generated/contracts.ts";
import type { ProjectItems } from "../../shared/project-groups.ts";

export function WorkflowProjectGroup({
  project,
  onRunUpdated,
}: {
  project: ProjectItems<WorkflowSummary>;
  onRunUpdated?: (workflow: WorkflowSummary, run: WorkflowRunSummary) => void;
}) {
  const active = project.items.filter((workflow) => workflow.state === "active").length;
  const hasFailures = project.items.some((workflow) => {
    const latest = workflow.runs[0];
    return latest?.lifecycle === "completed" && latest.outcome === "failure";
  });
  const hasRunning = project.items.some((workflow) => workflow.runs[0]?.lifecycle === "running");
  return (
    <ProjectGroup
      project={project}
      itemCount={project.items.length}
      defaultExpanded={hasFailures}
      status={hasFailures ? "failure" : hasRunning ? "running" : "default"}
      countLabel={`${project.items.length} workflow${project.items.length === 1 ? "" : "s"}`}
      metadata={`${active} active`}
    >
      {project.items.map((workflow) => (
        <WorkflowRow key={workflow.id} workflow={workflow} onRunUpdated={onRunUpdated} />
      ))}
    </ProjectGroup>
  );
}
