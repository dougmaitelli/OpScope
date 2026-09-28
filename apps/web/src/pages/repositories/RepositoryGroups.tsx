import { RepositoryRow, type RepositoryRowModel } from "../../components/RepositoryRow.tsx";
import "./RepositoryGroups.css";

export function RepositoryGroups({
  repositories,
  selectedIds,
  disabled,
  action,
  onSelectionChanged,
}: {
  repositories: RepositoryRowModel[];
  selectedIds: Set<string>;
  disabled: boolean;
  action: "add" | "remove";
  onSelectionChanged(id: string, selected: boolean): void;
}) {
  const groups = new Map<string, [RepositoryRowModel, ...RepositoryRowModel[]]>();
  for (const model of repositories) {
    const key = JSON.stringify([model.source.id, model.repository.owner]);
    const group = groups.get(key);
    if (group) group.push(model);
    else groups.set(key, [model]);
  }
  return (
    <div className="repository-groups">
      {[...groups.entries()]
        .sort(
          ([, a], [, b]) =>
            a[0].source.name.localeCompare(b[0].source.name) ||
            a[0].repository.owner.localeCompare(b[0].repository.owner),
        )
        .map(([key, models]) => (
          <section
            className="repository-owner-group"
            key={key}
            aria-label={`${models[0].source.name}: ${models[0].repository.owner}`}
          >
            <header>
              <h3>{models[0].repository.owner}</h3>
              <span>
                {models[0].source.name} · {models.length} repositories
              </span>
            </header>
            {models
              .sort((a, b) => a.repository.name.localeCompare(b.repository.name))
              .map((model) => (
                <RepositoryRow
                  key={model.selectionId}
                  model={model}
                  selected={selectedIds.has(model.selectionId)}
                  disabled={disabled}
                  action={action}
                  onSelectionChanged={(selected) => onSelectionChanged(model.selectionId, selected)}
                />
              ))}
          </section>
        ))}
    </div>
  );
}
