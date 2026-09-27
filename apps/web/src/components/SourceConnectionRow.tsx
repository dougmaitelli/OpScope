import type { ConnectionSummary, SourceSummary } from "../generated/contracts.ts";
import "./SourceConnectionRow.css";

interface SourceConnectionRowProps {
  source: SourceSummary;
  connection: ConnectionSummary;
  awaitingDisconnect: boolean;
  disconnecting: boolean;
  onConfigure(connection: ConnectionSummary): void;
  onRequestDisconnect(connection: ConnectionSummary): void;
  onCancelDisconnect(): void;
  onDisconnect(connection: ConnectionSummary): Promise<void>;
}

export function SourceConnectionRow({
  source,
  connection,
  awaitingDisconnect,
  disconnecting,
  onConfigure,
  onRequestDisconnect,
  onCancelDisconnect,
  onDisconnect,
}: SourceConnectionRowProps) {
  return (
    <div className="source-connection">
      <div className="connected-account">
        <span className="account-mark" aria-hidden="true">{source.abbreviation}</span>
        <div className="connection-account-copy">
          <strong>{connection.label}</strong>
          <span>
            {connection.handle
              ? `${connection.name} · ${connection.handle}`
              : connection.name}
          </span>
        </div>
        <span className="secure-label">Credential stored securely</span>
        <div className="source-actions">
          <button
            className="secondary-button compact-button"
            type="button"
            onClick={() => onConfigure(connection)}
          >
            Replace credential
          </button>
          <button
            className="danger-button compact-button"
            type="button"
            onClick={() => onRequestDisconnect(connection)}
          >
            Disconnect
          </button>
        </div>
      </div>

      {awaitingDisconnect && (
        <div className="disconnect-confirmation">
          <p>Remove the {connection.label} connection and its stored credential?</p>
          <div>
            <button
              className="secondary-button compact-button"
              type="button"
              onClick={onCancelDisconnect}
            >
              Cancel
            </button>
            <button
              className="danger-button compact-button"
              type="button"
              disabled={disconnecting}
              onClick={() => void onDisconnect(connection)}
            >
              {disconnecting ? "Removing…" : "Remove connection"}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
