import "./WorkflowFilters.css";

export type WorkflowStatusFilter =
  | "all"
  | "failing"
  | "running"
  | "successful"
  | "other";

interface WorkflowFiltersProps {
  query: string;
  status: WorkflowStatusFilter;
  onQueryChange: (query: string) => void;
  onStatusChange: (status: WorkflowStatusFilter) => void;
  onClear: () => void;
}

export function WorkflowFilters({
  query,
  status,
  onQueryChange,
  onStatusChange,
  onClear,
}: WorkflowFiltersProps) {
  const active = query.trim().length > 0 || status !== "all";

  return (
    <div className="workflow-filters" role="search" aria-label="Filter workflows">
      <label className="workflow-search">
        <span>Search</span>
        <input
          type="search"
          value={query}
          placeholder="Repository or workflow"
          onChange={(event) => onQueryChange(event.target.value)}
        />
      </label>
      <label className="workflow-status-filter">
        <span>Status</span>
        <select
          value={status}
          onChange={(event) =>
            onStatusChange(event.target.value as WorkflowStatusFilter)
          }
        >
          <option value="all">All statuses</option>
          <option value="failing">Failing</option>
          <option value="running">Running</option>
          <option value="successful">Successful</option>
          <option value="other">Other</option>
        </select>
      </label>
      <button
        className="secondary-button compact-button workflow-filters-clear"
        type="button"
        disabled={!active}
        onClick={onClear}
      >
        Clear
      </button>
    </div>
  );
}
