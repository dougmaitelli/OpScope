import "./ActivityFilters.css";

export type ActivityStatusFilter = "all" | "failing" | "running" | "successful" | "other";
export type ActivityRangeFilter = "all" | "day" | "week" | "month";
export type ActivityTypeFilter = "all" | "workflows" | "pullRequests";

export interface ActivityFilterValues {
  query: string;
  type: ActivityTypeFilter;
  status: ActivityStatusFilter;
  repository: string;
  trigger: string;
  range: ActivityRangeFilter;
}

interface ActivityFiltersProps {
  values: ActivityFilterValues;
  repositories: Array<{ id: string; label: string }>;
  triggers: string[];
  onChange: (values: ActivityFilterValues) => void;
  onClear: () => void;
}

export function ActivityFilters({
  values,
  repositories,
  triggers,
  onChange,
  onClear,
}: ActivityFiltersProps) {
  const active =
    values.query.trim().length > 0 ||
    values.type !== "all" ||
    values.status !== "all" ||
    values.repository.length > 0 ||
    values.trigger.length > 0 ||
    values.range !== "all";

  return (
    <div className="activity-filters" role="search" aria-label="Filter activity">
      <label className="activity-filter activity-filter-search">
        <span>Search</span>
        <input
          type="search"
          value={values.query}
          placeholder="Repository, workflow, pull request, actor"
          onChange={(event) => onChange({ ...values, query: event.target.value })}
        />
      </label>
      <label className="activity-filter">
        <span>Type</span>
        <select
          value={values.type}
          onChange={(event) =>
            onChange({
              ...values,
              type: event.target.value as ActivityTypeFilter,
              trigger: event.target.value === "pullRequests" ? "" : values.trigger,
            })
          }
        >
          <option value="all">All activity</option>
          <option value="workflows">Workflows</option>
          <option value="pullRequests">Pull requests</option>
        </select>
      </label>
      <label className="activity-filter">
        <span>Repository</span>
        <select
          value={values.repository}
          onChange={(event) => onChange({ ...values, repository: event.target.value })}
        >
          <option value="">All repositories</option>
          {repositories.map((repository) => (
            <option value={repository.id} key={repository.id}>
              {repository.label}
            </option>
          ))}
        </select>
      </label>
      <label className="activity-filter">
        <span>Status</span>
        <select
          value={values.status}
          onChange={(event) =>
            onChange({
              ...values,
              status: event.target.value as ActivityStatusFilter,
            })
          }
        >
          <option value="all">All statuses</option>
          <option value="failing">Failing</option>
          <option value="running">Running</option>
          <option value="successful">Successful</option>
          <option value="other">Other</option>
        </select>
      </label>
      <label className="activity-filter">
        <span>Trigger</span>
        <select
          value={values.trigger}
          disabled={values.type === "pullRequests"}
          onChange={(event) => onChange({ ...values, trigger: event.target.value })}
        >
          <option value="">All triggers</option>
          {triggers.map((trigger) => (
            <option value={trigger} key={trigger}>
              {trigger}
            </option>
          ))}
        </select>
      </label>
      <label className="activity-filter">
        <span>When</span>
        <select
          value={values.range}
          onChange={(event) =>
            onChange({ ...values, range: event.target.value as ActivityRangeFilter })
          }
        >
          <option value="all">Any time</option>
          <option value="day">Last 24 hours</option>
          <option value="week">Last 7 days</option>
          <option value="month">Last 30 days</option>
        </select>
      </label>
      <button
        className="secondary-button compact-button activity-filters-clear"
        type="button"
        disabled={!active}
        onClick={onClear}
      >
        Clear
      </button>
    </div>
  );
}
