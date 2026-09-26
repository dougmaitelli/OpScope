import type { SourceSummary } from "../generated/contracts.ts";
import "./SourceCard.css";

interface SourceCardProps {
  source: SourceSummary;
  awaitingDisconnect: boolean;
  disconnecting: boolean;
  onConfigure(source: SourceSummary): void;
  onRequestDisconnect(source: SourceSummary): void;
  onCancelDisconnect(): void;
  onDisconnect(source: SourceSummary): Promise<void>;
}

export function SourceCard({
  source,
  awaitingDisconnect,
  disconnecting,
  onConfigure,
  onRequestDisconnect,
  onCancelDisconnect,
  onDisconnect,
}: SourceCardProps) {
  const connection = source.connection;
  return (
    <article className="source-entry">
      <div className="source-row">
        <div className="source-identity">
          <span className="source-mark" aria-hidden="true">{source.abbreviation}</span>
          <div>
            <h3>{source.name}</h3>
            <p>{source.description}</p>
          </div>
        </div>
        <div className="source-state">
          <div className="connection-status">
            <span
              className={`status-dot ${connection ? "status-dot-connected" : "status-dot-muted"}`}
              aria-hidden="true"
            />
            <span>{connection ? "Connected" : "Not connected"}</span>
          </div>
          <div className="source-actions">
            <button
              className="secondary-button compact-button"
              type="button"
              onClick={() => onConfigure(source)}
            >
              {connection ? "Replace credential" : "Connect"}
            </button>
            {connection && (
              <button
                className="danger-button compact-button"
                type="button"
                onClick={() => onRequestDisconnect(source)}
              >
                Disconnect
              </button>
            )}
          </div>
        </div>
      </div>

      {connection && (
        <div className="connected-account">
          <span className="account-mark" aria-hidden="true">{source.abbreviation}</span>
          <div>
            <strong>
              {connection.handle
                ? `${connection.name} · ${connection.handle}`
                : connection.name}
            </strong>
            <span>Connected account</span>
          </div>
          <span className="secure-label">Credential stored securely</span>
        </div>
      )}

      {awaitingDisconnect && (
        <div className="disconnect-confirmation">
          <p>Remove the {source.name} connection and its stored credential?</p>
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
              onClick={() => void onDisconnect(source)}
            >
              {disconnecting ? "Removing…" : "Remove connection"}
            </button>
          </div>
        </div>
      )}
    </article>
  );
}
