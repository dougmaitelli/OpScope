import "./IssueFilters.css";

export function IssueFilters({
  query,
  repository,
  assignment,
  repositories,
  onQueryChange,
  onRepositoryChange,
  onAssignmentChange,
}: {
  query: string;
  repository: string;
  assignment: string;
  repositories: Array<{ id: string; label: string }>;
  onQueryChange: (value: string) => void;
  onRepositoryChange: (value: string) => void;
  onAssignmentChange: (value: string) => void;
}) {
  return (
    <div className="issue-filters">
      <input
        aria-label="Search issues"
        type="search"
        value={query}
        placeholder="Search title, repository, label, assignee, or author"
        onChange={(event) => onQueryChange(event.target.value)}
      />
      <select
        aria-label="Filter issues by repository"
        value={repository}
        onChange={(event) => onRepositoryChange(event.target.value)}
      >
        <option value="all">All repositories</option>
        {repositories.map((option) => (
          <option key={option.id} value={option.id}>
            {option.label}
          </option>
        ))}
      </select>
      <select
        aria-label="Filter issues by assignment"
        value={assignment}
        onChange={(event) => onAssignmentChange(event.target.value)}
      >
        <option value="all">All open issues</option>
        <option value="unassigned">Unassigned</option>
        <option value="assigned">Assigned</option>
      </select>
    </div>
  );
}
