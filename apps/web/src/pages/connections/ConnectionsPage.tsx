import { type FormEvent, useCallback, useEffect, useState } from "react";
import { useApplicationClient } from "../../api/application-client.tsx";
import { EmptyState } from "../../components/EmptyState.tsx";
import { PageHeader } from "../../components/PageHeader.tsx";
import { PanelHeader } from "../../components/PanelHeader.tsx";
import { SourceCard } from "../../components/SourceCard.tsx";
import type { SourceSummary } from "../../generated/contracts.ts";
import { requestErrorMessage } from "../../shared/errors.ts";

export function ConnectionsPage() {
  const client = useApplicationClient();
  const [sources, setSources] = useState<SourceSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState("");
  const [activeSourceId, setActiveSourceId] = useState<string | null>(null);
  const [pendingDisconnectId, setPendingDisconnectId] = useState<string | null>(null);
  const [disconnectingId, setDisconnectingId] = useState<string | null>(null);
  const [credential, setCredential] = useState("");
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
  const connectedCount = sources.filter((source) => source.connection !== null).length;

  const openEditor = (source: SourceSummary) => {
    setActiveSourceId(source.id);
    setPendingDisconnectId(null);
    setCredential("");
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
        credential: value,
      });
      setSources((current) =>
        current.map((source) =>
          source.id === activeSource.id ? { ...source, connection } : source,
        ),
      );
      setActiveSourceId(null);
      setCredential("");
      setNote(`${activeSource.name} connected.`);
    } catch (error: unknown) {
      setNoteError(true);
      setNote(requestErrorMessage(error));
    } finally {
      setConnecting(false);
    }
  };

  const disconnect = async (source: SourceSummary) => {
    setDisconnectingId(source.id);
    setNoteError(false);
    try {
      await client.disconnectSource({ sourceId: source.id });
      setSources((current) =>
        current.map((candidate) =>
          candidate.id === source.id ? { ...candidate, connection: null } : candidate,
        ),
      );
      setPendingDisconnectId(null);
      setNote(`${source.name} disconnected and its credential removed.`);
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
        description="Manage the services CI Watcher uses to discover monitoring data."
      />

      <section className="panel sources-panel" aria-labelledby="sources-heading">
        <PanelHeader
          label="Configured sources"
          title="Monitoring connections"
          metadata={<span className="source-count">{loadError ? "Unavailable" : `${connectedCount} connected`}</span>}
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
                awaitingDisconnect={pendingDisconnectId === source.id}
                disconnecting={disconnectingId === source.id}
                onConfigure={openEditor}
                onRequestDisconnect={(selected) => {
                  setPendingDisconnectId(selected.id);
                  setActiveSourceId(null);
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
              {activeSource.connection
                ? `Replace ${activeSource.name} credential`
                : `Connect ${activeSource.name}`}
            </h2>
            <p>CI Watcher validates the account before storing the credential securely.</p>
          </div>
          <form className="connection-form" aria-busy={connecting} onSubmit={(event) => void submit(event)}>
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
              {activeSource.credential.help} The credential is never returned to this interface after submission.
            </p>
            <div className="editor-actions">
              <button
                className="secondary-button"
                type="button"
                onClick={() => {
                  setActiveSourceId(null);
                  setCredential("");
                }}
              >
                Cancel
              </button>
              <button className="primary-button" type="submit" disabled={connecting}>
                {connecting
                  ? "Connecting…"
                  : activeSource.connection
                    ? "Replace credential"
                    : `Connect ${activeSource.name}`}
              </button>
            </div>
          </form>
        </section>
      )}
    </section>
  );
}
