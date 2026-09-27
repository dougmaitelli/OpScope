import { type FormEvent, useCallback, useEffect, useState } from "react";
import { useApplicationClient } from "../../api/application-client.tsx";
import { EmptyState } from "../../components/EmptyState.tsx";
import { PageHeader } from "../../components/PageHeader.tsx";
import { PanelHeader } from "../../components/PanelHeader.tsx";
import { SourceCard } from "../../components/SourceCard.tsx";
import type { ConnectionSummary, SourceSummary } from "../../generated/contracts.ts";
import { requestErrorMessage } from "../../shared/errors.ts";
import "./ConnectionsPage.css";

export function ConnectionsPage() {
  const client = useApplicationClient();
  const [sources, setSources] = useState<SourceSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState("");
  const [activeSourceId, setActiveSourceId] = useState<string | null>(null);
  const [activeConnectionId, setActiveConnectionId] = useState<string | null>(null);
  const [pendingDisconnectId, setPendingDisconnectId] = useState<string | null>(null);
  const [disconnectingId, setDisconnectingId] = useState<string | null>(null);
  const [credential, setCredential] = useState("");
  const [configuration, setConfiguration] = useState<Record<string, string>>({});
  const [connecting, setConnecting] = useState(false);
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

  const activeSource = sources.find((source) => source.id === activeSourceId) ?? null;
  const activeConnection =
    activeSource?.connections.find((connection) => connection.id === activeConnectionId) ?? null;
  const connectedCount = sources.reduce((total, source) => total + source.connections.length, 0);

  const openEditor = (source: SourceSummary, connection?: ConnectionSummary) => {
    setActiveSourceId(source.id);
    setActiveConnectionId(connection?.id ?? null);
    setPendingDisconnectId(null);
    setCredential("");
    setConfiguration(
      connection?.configuration ??
        Object.fromEntries(source.connectionFields.map((field) => [field.key, field.defaultValue])),
    );
  };

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const value = credential.trim();
    if (!activeSource || !value) {
      return;
    }
    setConnecting(true);
    setNoteError(false);
    setNote(`Contacting ${activeSource.name}…`);
    try {
      const connection = await client.connectSource({
        sourceId: activeSource.id,
        connectionId: activeConnection?.id ?? null,
        configuration,
        credential: value,
      });
      setSources((current) =>
        current.map((source) =>
          source.id === activeSource.id
            ? {
                ...source,
                connections: activeConnection
                  ? source.connections.map((candidate) =>
                      candidate.id === connection.id ? connection : candidate,
                    )
                  : [...source.connections, connection],
              }
            : source,
        ),
      );
      setActiveSourceId(null);
      setActiveConnectionId(null);
      setCredential("");
      setConfiguration({});
      setNote(`${connection.label} connected.`);
    } catch (error: unknown) {
      setNoteError(true);
      setNote(requestErrorMessage(error));
    } finally {
      setConnecting(false);
    }
  };

  const disconnect = async (connection: ConnectionSummary) => {
    setDisconnectingId(connection.id);
    setNoteError(false);
    try {
      await client.disconnectSource({ connectionId: connection.id });
      setSources((current) =>
        current.map((source) =>
          source.connections.some((candidate) => candidate.id === connection.id)
            ? {
                ...source,
                connections: source.connections.filter(
                  (candidate) => candidate.id !== connection.id,
                ),
              }
            : source,
        ),
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
    <section className="page-view" aria-labelledby="connections-title">
      <PageHeader
        eyebrow="Workspace settings"
        title="Connections"
        description="Manage the services OpsScope uses to discover monitoring data."
      />

      <section className="panel sources-panel" aria-labelledby="sources-heading">
        <PanelHeader
          label="Configured sources"
          title="Monitoring connections"
          metadata={
            <span className="source-count">
              {loadError ? "Unavailable" : `${connectedCount} connected`}
            </span>
          }
        />
        <div className="source-list" aria-live="polite">
          {loading ? (
            <EmptyState message="Loading source modules…" />
          ) : loadError ? (
            <EmptyState message={loadError} error />
          ) : sources.length === 0 ? (
            <EmptyState message="No source modules are registered." />
          ) : (
            sources.map((source) => (
              <SourceCard
                key={source.id}
                source={source}
                pendingDisconnectId={pendingDisconnectId}
                disconnectingId={disconnectingId}
                onConfigure={openEditor}
                onRequestDisconnect={(connection) => {
                  setPendingDisconnectId(connection.id);
                  setActiveSourceId(null);
                  setActiveConnectionId(null);
                }}
                onCancelDisconnect={() => setPendingDisconnectId(null)}
                onDisconnect={disconnect}
              />
            ))
          )}
        </div>
        <p
          className={`connection-note${noteError ? " connection-note-error" : ""}`}
          role="status"
          aria-live="polite"
        >
          {note}
        </p>
      </section>

      {activeSource && (
        <section className="panel connection-editor" aria-labelledby="connection-heading">
          <div className="editor-copy">
            <p className="section-label">Source credential</p>
            <h2 id="connection-heading">
              {activeConnection
                ? `Replace ${activeConnection?.label} credential`
                : `Add ${activeSource.name} connection`}
            </h2>
            <p>OpsScope validates the account before storing the credential securely.</p>
          </div>
          <form
            className="connection-form"
            aria-busy={connecting}
            onSubmit={(event) => void submit(event)}
          >
            {activeSource.connectionFields.map((field) => (
              <div className="connection-field" key={field.key}>
                <label htmlFor={`source-${field.key}`}>{field.label}</label>
                <input
                  id={`source-${field.key}`}
                  type="url"
                  autoComplete="off"
                  autoCapitalize="none"
                  spellCheck={false}
                  required
                  disabled={activeConnection !== null}
                  placeholder={field.placeholder}
                  value={configuration[field.key] ?? ""}
                  onChange={(event) =>
                    setConfiguration((current) => ({
                      ...current,
                      [field.key]: event.currentTarget.value,
                    }))
                  }
                />
                <p className="field-help">{field.help}</p>
              </div>
            ))}
            <label htmlFor="source-credential">{activeSource.credential.label}</label>
            <input
              id="source-credential"
              type="password"
              autoComplete="off"
              autoCapitalize="none"
              spellCheck={false}
              required
              autoFocus
              placeholder={activeSource.credential.placeholder}
              value={credential}
              onChange={(event) => setCredential(event.currentTarget.value)}
            />
            <p className="field-help">
              {activeSource.credential.help} The credential is never returned to this interface
              after submission.
            </p>
            <div className="editor-actions">
              <button
                className="secondary-button"
                type="button"
                onClick={() => {
                  setActiveSourceId(null);
                  setActiveConnectionId(null);
                  setCredential("");
                  setConfiguration({});
                }}
              >
                Cancel
              </button>
              <button className="primary-button" type="submit" disabled={connecting}>
                {connecting
                  ? "Connecting…"
                  : activeConnection
                    ? "Replace credential"
                    : "Add connection"}
              </button>
            </div>
          </form>
        </section>
      )}
    </section>
  );
}
