import type { ConnectionSummary, SourceSummary } from "../../generated/contracts.ts";
import "./ConnectionRow.css";

interface ConnectionRowProps {
  source: SourceSummary;
  connection: ConnectionSummary;
  awaitingDisconnect: boolean;
  disconnecting: boolean;
  disabled: boolean;
  onEdit(): void;
  onRequestDisconnect(): void;
  onCancelDisconnect(): void;
  onDisconnect(): void;
}

export function ConnectionRow({
  source,
  connection,
  awaitingDisconnect,
  disconnecting,
  disabled,
  onEdit,
  onRequestDisconnect,
  onCancelDisconnect,
  onDisconnect,
}: ConnectionRowProps) {
  return (
    <article className="connection-entry" aria-label={`${source.name} · ${connection.label}`}>
      <div className="connection-row">
        <div className="connection-provider">
          <span className="connection-provider-mark" aria-hidden="true">
            {source.abbreviation}
          </span>
          <span>{source.name}</span>
        </div>
        <div className="connection-identity">
          <strong>{connection.label}</strong>
          {source.connectionFields.map((field) =>
            connection.configuration[field.key] ? (
              <span key={field.key} title={field.label}>
                {connection.configuration[field.key]}
              </span>
            ) : null,
          )}
        </div>
        <div className="connection-account">
          <strong>{connection.name}</strong>
          {connection.handle ? <span>{connection.handle}</span> : null}
        </div>
        <div className="connection-row-actions">
          <button
            className="secondary-button compact-button"
            type="button"
            disabled={disabled}
            onClick={onEdit}
          >
            Replace credential
          </button>
          <button
            className="danger-button compact-button"
            type="button"
            disabled={disabled}
            onClick={onRequestDisconnect}
          >
            Disconnect
          </button>
        </div>
      </div>
      {awaitingDisconnect ? (
        <div className="connection-disconnect-confirmation">
          <p>Remove the {connection.label} connection and its stored credential?</p>
          <div>
            <button
              className="secondary-button compact-button"
              type="button"
              disabled={disabled}
              onClick={onCancelDisconnect}
            >
              Cancel
            </button>
            <button
              className={`danger-button compact-button${disconnecting ? " button-busy" : ""}`}
              type="button"
              disabled={disabled}
              aria-busy={disconnecting}
              onClick={onDisconnect}
            >
              {disconnecting ? "Removing…" : "Remove connection"}
            </button>
          </div>
        </div>
      ) : null}
    </article>
  );
}
