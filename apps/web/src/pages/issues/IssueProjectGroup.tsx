import { ProjectGroup } from "../../components/ProjectGroup.tsx";
import type { IssueSummary } from "../../generated/contracts.ts";
import type { ProjectItems } from "../../shared/project-groups.ts";
import { IssueListHeader, IssueRow } from "./IssueRow.tsx";

export function IssueProjectGroup({
  project,
  onOpen,
}: {
  project: ProjectItems<IssueSummary>;
  onOpen: (item: IssueSummary) => void;
}) {
  return (
    <ProjectGroup
      project={project}
      defaultExpanded
      itemCount={project.items.length}
      countLabel={`${project.items.length} issue${project.items.length === 1 ? "" : "s"}`}
    >
      <IssueListHeader />
      {project.items.map((item) => (
        <IssueRow key={item.id} issue={item} onOpen={() => onOpen(item)} />
      ))}
    </ProjectGroup>
  );
}
