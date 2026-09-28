import type { RepositorySourceSummary, RepositorySummary } from "../generated/contracts.ts";
import { DataRow, DataRowIdentity } from "./data-row/DataRow.tsx";
import "./RepositoryRow.css";

export interface RepositoryRowModel {
  selectionId: string;
  source: RepositorySourceSummary;
  repository: RepositorySummary;
}

interface RepositoryRowProps {
  model: RepositoryRowModel;
  selected: boolean;
  disabled?: boolean;
  action?: "add" | "remove";
  onSelectionChanged(selected: boolean): void;
}

export function RepositoryRow({
  model,
  selected,
  disabled,
  action = "add",
  onSelectionChanged,
}: RepositoryRowProps) {
  const { repository, source } = model;
  return (
    <DataRow as="div" className={`repository-row${selected ? " repository-row-selected" : ""}`}>
      <label className="repository-selection">
        <input
          type="checkbox"
          checked={selected}
          disabled={disabled}
          aria-label={`Select ${repository.owner}/${repository.name} to ${action === "add" ? "monitor" : "stop monitoring"}`}
          onChange={(event) => onSelectionChanged(event.currentTarget.checked)}
        />
        <DataRowIdentity
          title={`${repository.owner}/${repository.name}`}
          metadata={repository.description ?? "No description"}
        />
        <span className="repository-metadata">
          <span className="repository-visibility">
            {repository.visibility === "private" ? "Private" : "Public"}
          </span>
          <span className="repository-source">
            {source.abbreviation} · {source.name}
          </span>
        </span>
      </label>
    </DataRow>
  );
}
