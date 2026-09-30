import { type FormEvent, useEffect, useRef, useState } from "react";
import { useApplicationClient } from "../../api/application-client.tsx";
import { DialogCloseButton } from "../../components/DialogCloseButton.tsx";
import type { ConnectionSummary, SourceSummary } from "../../generated/contracts.ts";
import { requestErrorMessage } from "../../shared/errors.ts";
import "./ConnectionDialog.css";

interface ConnectionDialogProps {
  sources: SourceSummary[];
  existing?: { source: SourceSummary; connection: ConnectionSummary };
  onSaved(sourceId: string, connection: ConnectionSummary): void;
  onClose(): void;
}

function defaults(source?: SourceSummary): Record<string, string> {
  return Object.fromEntries(
    source?.connectionFields.map((field) => [field.key, field.defaultValue]) ?? [],
  );
}

export function ConnectionDialog({ sources, existing, onSaved, onClose }: ConnectionDialogProps) {
  const client = useApplicationClient();
  const dialog = useRef<HTMLDialogElement>(null);
  const initialSource = existing?.source ?? (sources.length === 1 ? sources[0] : undefined);
  const [sourceId, setSourceId] = useState(initialSource?.id ?? "");
  const [configuration, setConfiguration] = useState(
    existing?.connection.configuration ?? defaults(initialSource),
  );
  const [credential, setCredential] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const source = sources.find((candidate) => candidate.id === sourceId);

  useEffect(() => {
    dialog.current?.showModal();
  }, []);

  const close = () => {
    if (!saving) dialog.current?.close();
  };
  const selectSource = (id: string) => {
    setSourceId(id);
    setConfiguration(defaults(sources.find((candidate) => candidate.id === id)));
    setCredential("");
    setError("");
  };

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!source || !credential.trim() || saving) return;
    setSaving(true);
    setError("");
    try {
      const connection = await client.connectSource({
        sourceId: source.id,
        connectionId: existing?.connection.id ?? null,
        configuration,
        credential: credential.trim(),
      });
      setCredential("");
      onSaved(source.id, connection);
    } catch (failure: unknown) {
      setError(requestErrorMessage(failure));
    } finally {
      setSaving(false);
    }
  }

  return (
    <dialog
      className="connection-dialog"
      ref={dialog}
      aria-labelledby="connection-dialog-title"
      onCancel={(event) => {
        event.preventDefault();
        close();
      }}
      onClose={onClose}
      onClick={(event) => {
        if (event.target !== event.currentTarget) return;
        const bounds = event.currentTarget.getBoundingClientRect();
        if (
          event.clientX < bounds.left ||
          event.clientX > bounds.right ||
          event.clientY < bounds.top ||
          event.clientY > bounds.bottom
        )
          close();
      }}
    >
      <header className="connection-dialog-header">
        <div>
          <h2 id="connection-dialog-title">{existing ? "Replace credential" : "Add connection"}</h2>
          <p>
            {existing
              ? `${existing.source.name} · ${existing.connection.label}`
              : "Choose a service and configure its connection."}
          </p>
        </div>
        <DialogCloseButton label="Close connection dialog" disabled={saving} onClick={close} />
      </header>
      <form onSubmit={(event) => void submit(event)} aria-busy={saving}>
        <fieldset disabled={saving} className="connection-dialog-fields">
          {!existing ? (
            <div className="connection-dialog-field">
              <label htmlFor="connection-source">Source</label>
              <select
                id="connection-source"
                value={sourceId}
                onChange={(event) => selectSource(event.target.value)}
                required
                autoFocus={!initialSource}
              >
                <option value="" disabled>
                  Select a source
                </option>
                {sources.map((candidate) => (
                  <option key={candidate.id} value={candidate.id}>
                    {candidate.name}
                  </option>
                ))}
              </select>
              {source ? <p className="connection-dialog-help">{source.description}</p> : null}
            </div>
          ) : null}
          {source ? (
            <>
              {source.connectionFields.map((field) => (
                <div className="connection-dialog-field" key={field.key}>
                  <label htmlFor={`connection-${field.key}`}>{field.label}</label>
                  <input
                    id={`connection-${field.key}`}
                    type={field.inputType}
                    autoComplete="off"
                    autoCapitalize="none"
                    spellCheck={false}
                    required
                    disabled={Boolean(existing)}
                    placeholder={field.placeholder}
                    value={configuration[field.key] ?? ""}
                    onChange={(event) => {
                      const value = event.currentTarget.value;
                      setConfiguration((current) => ({ ...current, [field.key]: value }));
                    }}
                  />
                  {!existing ? <p className="connection-dialog-help">{field.help}</p> : null}
                </div>
              ))}
              <div className="connection-dialog-field">
                <label htmlFor="connection-credential">{source.credential.label}</label>
                <input
                  id="connection-credential"
                  type="password"
                  autoComplete="off"
                  autoCapitalize="none"
                  spellCheck={false}
                  required
                  autoFocus={Boolean(initialSource)}
                  placeholder={source.credential.placeholder}
                  value={credential}
                  onChange={(event) => setCredential(event.currentTarget.value)}
                />
                <p className="connection-dialog-help">{source.credential.help}</p>
              </div>
            </>
          ) : null}
          {error ? (
            <p className="connection-dialog-error" role="alert">
              {error}
            </p>
          ) : null}
          <div className="connection-dialog-actions">
            <button className="secondary-button compact-button" type="button" onClick={close}>
              Cancel
            </button>
            <button
              className={`primary-button compact-button${saving ? " button-busy" : ""}`}
              type="submit"
              disabled={!source || !credential.trim() || saving}
            >
              {saving ? "Validating…" : existing ? "Replace credential" : "Add connection"}
            </button>
          </div>
        </fieldset>
      </form>
    </dialog>
  );
}
