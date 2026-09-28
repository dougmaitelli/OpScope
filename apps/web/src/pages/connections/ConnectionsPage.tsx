import { useCallback, useEffect, useState } from "react";
import { useApplicationClient } from "../../api/application-client.tsx";
import { EmptyState } from "../../components/EmptyState.tsx";
import { PageHeader } from "../../components/PageHeader.tsx";
import { PanelHeader } from "../../components/PanelHeader.tsx";
import type { ConnectionSummary, SourceSummary } from "../../generated/contracts.ts";
import { requestErrorMessage } from "../../shared/errors.ts";
import { ConnectionDialog } from "./ConnectionDialog.tsx";
import { ConnectionRow } from "./ConnectionRow.tsx";
import "./ConnectionsPage.css";

export function ConnectionsPage() {
  const client = useApplicationClient();
  const [sources, setSources] = useState<SourceSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState("");
  const [editor, setEditor] = useState<
    { source: SourceSummary; connection: ConnectionSummary } | "new" | null
  >(null);
  const [pendingDisconnectId, setPendingDisconnectId] = useState<string | null>(null);
  const [disconnectingId, setDisconnectingId] = useState<string | null>(null);
  const [note, setNote] = useState("");
  const [noteError, setNoteError] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const response = await client.listSources();
      setSources(response.sources);
      setLoadError("");
    } catch (error: unknown) {
      setLoadError(requestErrorMessage(error));
    } finally {
      setLoading(false);
    }
  }, [client]);

  useEffect(() => {
    void load();
  }, [load]);

  const connections = sources.flatMap((source) =>
    source.connections.map((connection) => ({ source, connection })),
  );
  const canAdd = !loading && !loadError && sources.length > 0 && disconnectingId === null;

  const openAdd = () => {
    setPendingDisconnectId(null);
    setNote("");
    setEditor("new");
  };

  const saved = (sourceId: string, connection: ConnectionSummary) => {
    setSources((current) =>
      current.map((source) =>
        source.id === sourceId
          ? {
              ...source,
              connections: source.connections.some((candidate) => candidate.id === connection.id)
                ? source.connections.map((candidate) =>
                    candidate.id === connection.id ? connection : candidate,
                  )
                : [...source.connections, connection],
            }
          : source,
      ),
    );
    setNoteError(false);
    setNote(
      editor === "new"
        ? `${connection.label} connected.`
        : `${connection.label} credential updated.`,
    );
    setEditor(null);
  };

  const disconnect = async (connection: ConnectionSummary) => {
    if (disconnectingId) return;
    setDisconnectingId(connection.id);
    setNote("");
    setNoteError(false);
    try {
      await client.disconnectSource({ connectionId: connection.id });
      setSources((current) =>
        current.map((source) => ({
          ...source,
          connections: source.connections.filter((candidate) => candidate.id !== connection.id),
        })),
      );
      setPendingDisconnectId(null);
      setNote(`${connection.label} disconnected and its credential removed.`);
    } catch (error: unknown) {
      setNoteError(true);
      setNote(requestErrorMessage(error));
    } finally {
      setDisconnectingId(null);
    }
  };

  return (
    <section className="page-view" aria-label="Connections">
      <PageHeader
        eyebrow="Workspace settings"
        title="Connections"
        description="Manage the services OpsScope uses to discover monitoring data."
        actions={
          <button
            className="primary-button compact-button connection-add-button"
            type="button"
            disabled={!canAdd}
            onClick={openAdd}
          >
            Add connection
          </button>
        }
      />
      <section className="panel connections-panel" aria-label="Configured connections">
        <PanelHeader
          label="Configured services"
          title="Connections"
          metadata={
            <span className="connection-count">
              {loadError ? "Unavailable" : `${connections.length} configured`}
            </span>
          }
        />
        {loading ? (
          <EmptyState message="Loading connections…" />
        ) : loadError ? (
          <div className="connections-empty">
            <EmptyState message={loadError} error />
            <button
              type="button"
              className="secondary-button compact-button"
              onClick={() => void load()}
            >
              Retry
            </button>
          </div>
        ) : connections.length === 0 ? (
          <div className="connections-empty">
            <EmptyState
              message={
                sources.length
                  ? "No connections yet. Add a service to start monitoring your repositories."
                  : "No source modules are available."
              }
            />
            {sources.length > 0 ? (
              <button className="secondary-button compact-button" type="button" onClick={openAdd}>
                Add connection
              </button>
            ) : null}
          </div>
        ) : (
          <>
            <div className="connections-list-header" aria-hidden="true">
              <span>Provider</span>
              <span>Connection / server</span>
              <span>Account</span>
              <span />
            </div>
            <div className="connections-list">
              {connections.map(({ source, connection }) => (
                <ConnectionRow
                  key={connection.id}
                  source={source}
                  connection={connection}
                  awaitingDisconnect={pendingDisconnectId === connection.id}
                  disconnecting={disconnectingId === connection.id}
                  disabled={disconnectingId !== null}
                  onEdit={() => {
                    setPendingDisconnectId(null);
                    setNote("");
                    setEditor({ source, connection });
                  }}
                  onRequestDisconnect={() => {
                    setNote("");
                    setPendingDisconnectId(connection.id);
                  }}
                  onCancelDisconnect={() => setPendingDisconnectId(null)}
                  onDisconnect={() => void disconnect(connection)}
                />
              ))}
            </div>
          </>
        )}
        {note ? (
          <p
            className={`connection-note${noteError ? " connection-note-error" : ""}`}
            role={noteError ? "alert" : "status"}
          >
            {note}
          </p>
        ) : null}
      </section>
      {editor ? (
        <ConnectionDialog
          sources={sources}
          existing={editor === "new" ? undefined : editor}
          onSaved={saved}
          onClose={() => setEditor(null)}
        />
      ) : null}
    </section>
  );
}
