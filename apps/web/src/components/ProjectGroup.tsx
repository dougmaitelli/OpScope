import { useState, type ComponentPropsWithoutRef, type ReactNode } from "react";
import type { ProjectIdentity } from "../shared/project-groups.ts";
import "./ProjectGroup.css";

export function ProjectGroup({
  project,
  countLabel,
  itemCount,
  defaultExpanded = false,
  metadata,
  children,
}: {
  project: ProjectIdentity;
  countLabel: string;
  itemCount: number;
  defaultExpanded?: boolean;
  metadata?: ReactNode;
  children: ReactNode;
}) {
  // Initial state only: refreshes should not override the user's expand/collapse choice.
  const [expanded, setExpanded] = useState(defaultExpanded);
  if (itemCount === 0) return null;

  return (
    <details
      className="project-group"
      open={expanded}
      onToggle={(event) => setExpanded(event.currentTarget.open)}
    >
      <summary className="project-header">
        <div className="project-identity">
          <h3>
            {project.repositoryOwner}/{project.repositoryName}
          </h3>
          <p>
            {project.sourceName} · {countLabel}
          </p>
        </div>
        <div className="project-metadata">
          {metadata ? <span className="project-summary">{metadata}</span> : null}
          <span className="project-chevron" aria-hidden="true">
            <svg viewBox="0 0 16 16">
              <path d="m4.5 6 3.5 3.5L11.5 6" />
            </svg>
          </span>
        </div>
      </summary>
      <div className="project-items">{children}</div>
    </details>
  );
}

export function ProjectGroupList({ className = "", ...props }: ComponentPropsWithoutRef<"div">) {
  return <div {...props} className={`project-group-list ${className}`} />;
}
