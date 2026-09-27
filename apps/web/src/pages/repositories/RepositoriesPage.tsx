import { useCallback, useEffect, useMemo, useState } from "react";
import { useApplicationClient } from "../../api/application-client.tsx";
import { EmptyState } from "../../components/EmptyState.tsx";
import { PageHeader } from "../../components/PageHeader.tsx";
import { PanelHeader } from "../../components/PanelHeader.tsx";
import { RepositoryRow, type RepositoryRowModel } from "../../components/RepositoryRow.tsx";
import type { RepositorySourceSummary } from "../../generated/contracts.ts";
import { requestErrorMessage } from "../../shared/errors.ts";
import "./RepositoriesPage.css";

function selectionsMatch(left: Set<string>, right: Set<string>): boolean {
  return left.size === right.size && [...left].every((id) => right.has(id));
}

export function RepositoriesPage() {
  const client = useApplicationClient();
  const [sources, setSources] = useState<RepositorySourceSummary[]>([]);
  const [selectedIds, setSelectedIds] = useState(new Set<string>());
  const [appliedIds, setAppliedIds] = useState(new Set<string>());
  const [query, setQuery] = useState("");
  const [note, setNote] = useState("Repository selections are stored locally.");
  const [loadError, setLoadError] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

  const allRepositories = useMemo<RepositoryRowModel[]>(
    () =>
      sources.flatMap((source) =>
        source.repositories.map((repository) => ({
          selectionId: `${source.id}:${repository.id}`,
          source,
          repository,
        })),
      ),
    [sources],
  );

  const visibleRepositories = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    if (!normalized) {
      return allRepositories;
    }
    return allRepositories.filter(({ repository, source }) =>
      `${repository.owner}/${repository.name} ${repository.description ?? ""} ${source.name}`
        .toLocaleLowerCase()
        .includes(normalized),
    );
  }, [allRepositories, query]);

  const load = useCallback(async () => {
    setLoading(true);
    setLoadError("");
    try {
      const response = await client.listRepositories();
      const selected = new Set(
        response.sources.flatMap((source) =>
          source.repositories
            .filter((repository) => repository.selected)
            .map((repository) => `${source.id}:${repository.id}`),
        ),
      );
      setSources(response.sources);
      setSelectedIds(selected);
      setAppliedIds(new Set(selected));
      setNote("Repository selections are stored locally.");
    } catch (error: unknown) {
      setSources([]);
      setLoadError(requestErrorMessage(error));
      setNote("Repository discovery failed.");
    } finally {
      setLoading(false);
    }
  }, [client]);

  useEffect(() => {
    void load();
  }, [load]);

  const changeSelection = (selectionId: string, selected: boolean) => {
    setSelectedIds((current) => {
      const next = new Set(current);
      if (selected) {
        next.add(selectionId);
      } else {
        next.delete(selectionId);
      }
      return next;
    });
    setNote("Selection changed. Apply it for this session.");
  };

  const save = async () => {
    setSaving(true);
    setNote("Saving repository selection…");
    try {
      const response = await client.saveRepositorySelection({
        sources: sources.map((source) => ({
          sourceId: source.id,
          repositoryIds: source.repositories
            .filter((repository) => selectedIds.has(`${source.id}:${repository.id}`))
            .map((repository) => repository.id),
        })),
      });
      setAppliedIds(new Set(selectedIds));
      setNote(`${response.selectedCount} repositories saved for monitoring.`);
    } catch (error: unknown) {
      setNote(requestErrorMessage(error));
    } finally {
      setSaving(false);
    }
  };

  const unchanged = selectionsMatch(selectedIds, appliedIds);
  const allVisibleSelected =
    visibleRepositories.length > 0 &&
    visibleRepositories.every(({ selectionId }) => selectedIds.has(selectionId));

  return (
    <section className="page-view" aria-labelledby="repositories-title">
      <PageHeader
        eyebrow="Monitoring scope"
        title="Repositories"
        description="Choose which repositories CI Watcher should monitor."
        actions={
          <button
            className={`secondary-button${loading ? " button-busy" : ""}`}
            type="button"
            disabled={loading}
            onClick={() => void load()}
          >
            <span aria-hidden="true">↻</span>
            Refresh
          </button>
        }
      />

      <section className="panel repositories-panel" aria-labelledby="repository-list-heading">
        <PanelHeader
          label="Available repositories"
          title="Repository selection"
          metadata={<span className="panel-badge">Live discovery</span>}
        />
        <div className="repository-toolbar">
          <label className="repository-search">
            <span>Search repositories</span>
            <input
              type="search"
              placeholder="Search by owner or repository"
              autoComplete="off"
              value={query}
              disabled={loading || Boolean(loadError)}
              onChange={(event) => setQuery(event.currentTarget.value)}
            />
          </label>
          <div className="repository-selection-controls">
            <button
              className="secondary-button compact-button"
              type="button"
              disabled={loading || Boolean(loadError) || allVisibleSelected}
              onClick={() => {
                setSelectedIds(
                  (current) =>
                    new Set([
                      ...current,
                      ...visibleRepositories.map(({ selectionId }) => selectionId),
                    ]),
                );
                setNote("Visible repositories selected. Apply for this session.");
              }}
            >
              Select visible
            </button>
            <button
              className="secondary-button compact-button"
              type="button"
              disabled={loading || Boolean(loadError) || selectedIds.size === 0}
              onClick={() => {
                setSelectedIds(new Set());
                setNote("Selection cleared. Apply for this session.");
              }}
            >
              Clear
            </button>
          </div>
        </div>

        <div className="repository-list-heading">
          <span>
            {visibleRepositories.length} shown · {selectedIds.size} selected
          </span>
          <span>Monitor</span>
        </div>
        <div className="repository-list" aria-live="polite">
          {loading ? (
            <EmptyState message="Loading repositories…" />
          ) : loadError ? (
            <EmptyState message={loadError} error />
          ) : sources.length === 0 ? (
            <EmptyState message="Connect a source to discover repositories." />
          ) : allRepositories.length === 0 ? (
            <EmptyState message="No repositories are available to the connected account." />
          ) : visibleRepositories.length === 0 ? (
            <EmptyState message="No repositories match this search." />
          ) : (
            visibleRepositories.map((model) => (
              <RepositoryRow
                key={model.selectionId}
                model={model}
                selected={selectedIds.has(model.selectionId)}
                onSelectionChanged={(selected) => changeSelection(model.selectionId, selected)}
              />
            ))
          )}
        </div>

        <div className="repository-footer">
          <p role="status" aria-live="polite">
            {note}
          </p>
          <div className="repository-footer-actions">
            <button
              className="secondary-button"
              type="button"
              disabled={loading || unchanged || saving}
              onClick={() => {
                setSelectedIds(new Set(appliedIds));
                setNote("Selection reset to the last applied state.");
              }}
            >
              Reset
            </button>
            <button
              className="primary-button"
              type="button"
              disabled={loading || unchanged || saving}
              onClick={() => void save()}
            >
              {saving ? "Saving…" : "Apply selection"}
            </button>
          </div>
        </div>
      </section>
    </section>
  );
}
