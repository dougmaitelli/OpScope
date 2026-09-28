import { useCallback, useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { useApplicationClient } from "../../api/application-client.tsx";
import { EmptyState } from "../../components/EmptyState.tsx";
import { PageHeader } from "../../components/PageHeader.tsx";
import type { RepositoryRowModel } from "../../components/RepositoryRow.tsx";
import type { RepositorySourceSummary } from "../../generated/contracts.ts";
import { requestErrorMessage } from "../../shared/errors.ts";
import { RepositoryGroups } from "./RepositoryGroups.tsx";
import "./RepositoriesPage.css";

type RepositoryView = "monitored" | "browse";

export function RepositoriesPage() {
  const client = useApplicationClient();
  const [sources, setSources] = useState<RepositorySourceSummary[]>([]);
  const [appliedIds, setAppliedIds] = useState(new Set<string>());
  const [selectedIds, setSelectedIds] = useState(new Set<string>());
  const [view, setView] = useState<RepositoryView>("monitored");
  const [query, setQuery] = useState("");
  const [sourceId, setSourceId] = useState("");
  const [owner, setOwner] = useState("");
  const [note, setNote] = useState("");
  const [loadError, setLoadError] = useState("");
  const [saveError, setSaveError] = useState("");
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
  const candidates = useMemo(
    () =>
      allRepositories.filter(
        ({ selectionId }) => appliedIds.has(selectionId) === (view === "monitored"),
      ),
    [allRepositories, appliedIds, view],
  );
  const owners = useMemo(
    () =>
      [
        ...new Set(
          candidates
            .filter(({ source }) => !sourceId || source.id === sourceId)
            .map(({ repository }) => repository.owner),
        ),
      ].sort((a, b) => a.localeCompare(b)),
    [candidates, sourceId],
  );
  const visibleRepositories = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    return candidates.filter(
      ({ repository, source }) =>
        (!sourceId || source.id === sourceId) &&
        (!owner || repository.owner === owner) &&
        `${repository.owner}/${repository.name} ${repository.description ?? ""} ${source.name}`
          .toLocaleLowerCase()
          .includes(normalized),
    );
  }, [candidates, query, sourceId, owner]);

  const load = useCallback(async () => {
    setLoading(true);
    setLoadError("");
    try {
      const response = await client.listRepositories();
      setSources(response.sources);
      setAppliedIds(
        new Set(
          response.sources.flatMap((source) =>
            source.repositories
              .filter((repository) => repository.selected)
              .map((repository) => `${source.id}:${repository.id}`),
          ),
        ),
      );
      setSelectedIds(new Set());
      setNote("");
      setSaveError("");
      setSourceId("");
      setOwner("");
    } catch (error: unknown) {
      setLoadError(requestErrorMessage(error));
    } finally {
      setLoading(false);
    }
  }, [client]);

  useEffect(() => {
    void load();
  }, [load]);

  const changeView = (next: RepositoryView) => {
    if (next === view) return;
    setView(next);
    setSelectedIds(new Set());
    setQuery("");
    setSourceId("");
    setOwner("");
    setNote("");
    setSaveError("");
  };
  const changeSelection = (id: string, selected: boolean) => {
    setSelectedIds((current) => {
      const next = new Set(current);
      if (selected) next.add(id);
      else next.delete(id);
      return next;
    });
    setNote("");
    setSaveError("");
  };
  const save = async () => {
    const next = new Set(appliedIds);
    for (const id of selectedIds) {
      if (view === "browse") next.add(id);
      else next.delete(id);
    }
    setSaving(true);
    setSaveError("");
    setNote("");
    try {
      await client.saveRepositorySelection({
        sources: sources.map((source) => ({
          sourceId: source.id,
          repositoryIds: source.repositories
            .filter((repository) => next.has(`${source.id}:${repository.id}`))
            .map((repository) => repository.id),
        })),
      });
      setAppliedIds(next);
      setSelectedIds(new Set());
      setNote(
        `${selectedIds.size} repositories ${view === "browse" ? "added to" : "removed from"} monitoring.`,
      );
    } catch (error: unknown) {
      setSaveError(requestErrorMessage(error));
    } finally {
      setSaving(false);
    }
  };

  const busy = loading || saving;
  const allVisibleSelected =
    visibleRepositories.length > 0 &&
    visibleRepositories.every(({ selectionId }) => selectedIds.has(selectionId));
  const hiddenSelected = [...selectedIds].filter(
    (id) => !visibleRepositories.some(({ selectionId }) => selectionId === id),
  ).length;
  const hasFilters = Boolean(query || sourceId || owner);

  return (
    <section className="page-view" aria-labelledby="repositories-title">
      <PageHeader
        eyebrow="Monitoring scope"
        title="Repositories"
        description="Manage monitored repositories or add more from your connections."
        actions={
          <button
            className={`secondary-button${loading ? " button-busy" : ""}`}
            type="button"
            disabled={busy || selectedIds.size > 0}
            onClick={() => void load()}
          >
            <span aria-hidden="true">↻</span> Refresh
          </button>
        }
      />
      <section className="panel repositories-panel" aria-label="Repository management">
        <div className="repository-views" role="group" aria-label="Repository view">
          <button
            type="button"
            aria-pressed={view === "monitored"}
            disabled={busy}
            onClick={() => changeView("monitored")}
          >
            Monitored ({appliedIds.size})
          </button>
          <button
            type="button"
            aria-pressed={view === "browse"}
            disabled={busy}
            onClick={() => changeView("browse")}
          >
            Add repositories ({allRepositories.length - appliedIds.size})
          </button>
        </div>
        <div className="repository-toolbar">
          <label className="repository-search">
            <span>Search repositories</span>
            <input
              type="search"
              placeholder="Search by owner or repository"
              autoComplete="off"
              value={query}
              disabled={busy}
              onChange={(event) => setQuery(event.currentTarget.value)}
            />
          </label>
          <label className="repository-filter">
            <span>Connection</span>
            <select
              value={sourceId}
              disabled={busy}
              onChange={(event) => {
                setSourceId(event.currentTarget.value);
                setOwner("");
              }}
            >
              <option value="">All connections</option>
              {sources.map((source) => (
                <option key={source.id} value={source.id}>
                  {source.name}
                </option>
              ))}
            </select>
          </label>
          <label className="repository-filter">
            <span>Owner</span>
            <select
              value={owner}
              disabled={busy}
              onChange={(event) => setOwner(event.currentTarget.value)}
            >
              <option value="">All owners</option>
              {/* Keep a selected owner available even after its last repository is removed. */}
              {owner && !owners.includes(owner) ? <option value={owner}>{owner}</option> : null}
              {owners.map((value) => (
                <option key={value} value={value}>
                  {value}
                </option>
              ))}
            </select>
          </label>
          {hasFilters ? (
            <button
              className="secondary-button compact-button"
              type="button"
              disabled={busy}
              onClick={() => {
                setQuery("");
                setSourceId("");
                setOwner("");
              }}
            >
              Clear filters
            </button>
          ) : null}
        </div>
        <div className="repository-list-heading">
          <span>
            {visibleRepositories.length} shown · {selectedIds.size} selected
            {hiddenSelected ? ` (${hiddenSelected} hidden by filters)` : ""}
          </span>
          <div className="repository-selection-controls">
            <button
              className="secondary-button compact-button"
              type="button"
              disabled={
                busy || Boolean(loadError) || !visibleRepositories.length || allVisibleSelected
              }
              onClick={() =>
                setSelectedIds(
                  (current) =>
                    new Set([
                      ...current,
                      ...visibleRepositories.map(({ selectionId }) => selectionId),
                    ]),
                )
              }
            >
              Select visible
            </button>
            <button
              className="secondary-button compact-button"
              type="button"
              disabled={busy || !selectedIds.size}
              onClick={() => setSelectedIds(new Set())}
            >
              Clear selection
            </button>
          </div>
        </div>
        <div className="repository-list" aria-busy={busy}>
          {loading ? (
            <EmptyState message="Loading repositories…" />
          ) : loadError ? (
            <EmptyState message={loadError} error />
          ) : sources.length === 0 ? (
            <div className="repository-empty">
              <EmptyState message="Connect a source to discover repositories." />
              <Link className="secondary-button" to="/connections">
                Manage connections
              </Link>
            </div>
          ) : view === "monitored" && appliedIds.size === 0 ? (
            <div className="repository-empty">
              <EmptyState message="No repositories are monitored yet." />
              <button
                className="secondary-button"
                type="button"
                onClick={() => changeView("browse")}
              >
                Add repositories
              </button>
            </div>
          ) : visibleRepositories.length === 0 ? (
            <EmptyState
              message={
                hasFilters
                  ? "No repositories match these filters."
                  : view === "browse" && allRepositories.length > 0
                    ? "All available repositories are already monitored."
                    : "No repositories are available to the connected accounts."
              }
            />
          ) : (
            <RepositoryGroups
              repositories={visibleRepositories}
              selectedIds={selectedIds}
              disabled={busy}
              action={view === "browse" ? "add" : "remove"}
              onSelectionChanged={changeSelection}
            />
          )}
        </div>
        <div className="repository-footer">
          <p
            role={saveError ? "alert" : "status"}
            className={saveError ? "repository-error" : undefined}
          >
            {saveError ||
              note ||
              (view === "browse"
                ? "Only repositories not yet monitored are shown. Select repositories to add."
                : "Select repositories to stop monitoring.")}
          </p>
          <div className="repository-footer-actions">
            <button
              className={`primary-button${saving ? " button-busy" : ""}`}
              type="button"
              disabled={busy || Boolean(loadError) || !selectedIds.size}
              onClick={() => void save()}
            >
              {saving
                ? "Saving…"
                : view === "browse"
                  ? `Add selected${selectedIds.size ? ` (${selectedIds.size})` : ""}`
                  : `Stop monitoring${selectedIds.size ? ` (${selectedIds.size})` : ""}`}
            </button>
          </div>
        </div>
      </section>
    </section>
  );
}
