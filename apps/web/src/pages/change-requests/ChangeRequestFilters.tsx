import "./ChangeRequestFilters.css";

export type ChangeRequestFilter = "all" | "attention" | "ready" | "draft";

export function ChangeRequestFilters({
  query,
  status,
  onQueryChange,
  onStatusChange,
}: {
  query: string;
  status: ChangeRequestFilter;
  onQueryChange: (value: string) => void;
  onStatusChange: (value: ChangeRequestFilter) => void;
}) {
  return (
    <div className="change-request-filters">
      <input
        aria-label="Search pull requests"
        type="search"
        value={query}
        placeholder="Search title, repository, branch, or author"
        onChange={(event) => onQueryChange(event.target.value)}
      />
      <select
        aria-label="Filter pull requests by status"
        value={status}
        onChange={(event) => onStatusChange(event.target.value as ChangeRequestFilter)}
      >
        <option value="all">All open</option>
        <option value="attention">Needs attention</option>
        <option value="ready">Ready</option>
        <option value="draft">Draft</option>
      </select>
    </div>
  );
}
