import { ProjectGroup } from "../../components/ProjectGroup.tsx";
import type { ChangeRequestSummary } from "../../generated/contracts.ts";
import type { ProjectItems } from "../../shared/project-groups.ts";
import { ChangeRequestListHeader, ChangeRequestRow } from "./ChangeRequestRow.tsx";

export function ChangeRequestProjectGroup({
  project,
  onOpen,
}: {
  project: ProjectItems<ChangeRequestSummary>;
  onOpen: (item: ChangeRequestSummary) => void;
}) {
  const hasFailures = project.items.some(
    (item) =>
      item.checkStatus === "failing" ||
      item.reviewStatus === "changesRequested" ||
      item.mergeStatus === "conflicting",
  );
  const hasRunning = project.items.some((item) => item.checkStatus === "running");
  return (
    <ProjectGroup
      project={project}
      defaultExpanded
      status={hasFailures ? "failure" : hasRunning ? "running" : "default"}
      itemCount={project.items.length}
      countLabel={`${project.items.length} pull request${project.items.length === 1 ? "" : "s"}`}
    >
      <ChangeRequestListHeader />
      {project.items.map((item) => (
        <ChangeRequestRow key={item.id} changeRequest={item} onOpen={() => onOpen(item)} />
      ))}
    </ProjectGroup>
  );
}
