import type {
  RepositorySourceSummary,
  RepositorySummary,
} from "../generated/contracts.ts";
import "./RepositoryRow.css";

export interface RepositoryRowModel {
  selectionId: string;
  source: RepositorySourceSummary;
  repository: RepositorySummary;
}

interface RepositoryRowProps {
  model: RepositoryRowModel;
  selected: boolean;
  onSelectionChanged(selected: boolean): void;
}

export function RepositoryRow({ model, selected, onSelectionChanged }: RepositoryRowProps) {
  const { repository, source } = model;
  return (
    <label className={`repository-row${selected ? " repository-row-selected" : ""}`}>
      <input
        type="checkbox"
        checked={selected}
        aria-label={`Monitor ${repository.owner}/${repository.name}`}
        onChange={(event) => onSelectionChanged(event.currentTarget.checked)}
      />
      <span className="repository-mark" aria-hidden="true">
        {repository.name.slice(0, 2).toUpperCase()}
      </span>
      <span className="repository-identity">
        <strong>{repository.owner}/{repository.name}</strong>
        <span>{repository.description ?? "No description"}</span>
      </span>
      <span className="repository-metadata">
        <span className="repository-visibility">
          {repository.visibility === "private" ? "Private" : "Public"}
        </span>
        <span className="repository-source">{source.abbreviation} · {source.name}</span>
      </span>
    </label>
  );
}
