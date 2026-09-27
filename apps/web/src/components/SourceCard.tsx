import type { ConnectionSummary, SourceSummary } from "../generated/contracts.ts";
import { SourceConnectionRow } from "./SourceConnectionRow.tsx";
import "./SourceCard.css";

interface SourceCardProps {
  source: SourceSummary;
  pendingDisconnectId: string | null;
  disconnectingId: string | null;
  onConfigure(source: SourceSummary, connection?: ConnectionSummary): void;
  onRequestDisconnect(connection: ConnectionSummary): void;
  onCancelDisconnect(): void;
  onDisconnect(connection: ConnectionSummary): Promise<void>;
}

export function SourceCard({
  source,
  pendingDisconnectId,
  disconnectingId,
  onConfigure,
  onRequestDisconnect,
  onCancelDisconnect,
  onDisconnect,
}: SourceCardProps) {
  const connectionCount = source.connections.length;
  return (
    <article className="source-entry">
      <div className="source-row">
        <div className="source-identity">
          <span className="source-mark" aria-hidden="true">
            {source.abbreviation}
          </span>
          <div>
            <h3>{source.name}</h3>
            <p>{source.description}</p>
          </div>
        </div>
        <div className="source-state">
          <div className="connection-status">
            <span
              className={`status-dot ${connectionCount > 0 ? "status-dot-connected" : "status-dot-muted"}`}
              aria-hidden="true"
            />
            <span>{connectionCount === 0 ? "Not connected" : `${connectionCount} connected`}</span>
          </div>
          <div className="source-actions">
            <button
              className="secondary-button compact-button"
              type="button"
              onClick={() => onConfigure(source)}
            >
              Add connection
            </button>
          </div>
        </div>
      </div>

      <div className="source-connections">
        {source.connections.length === 0 ? (
          <p className="no-connections">No connections configured for this module.</p>
        ) : (
          source.connections.map((connection) => (
            <SourceConnectionRow
              key={connection.id}
              source={source}
              connection={connection}
              awaitingDisconnect={pendingDisconnectId === connection.id}
              disconnecting={disconnectingId === connection.id}
              onConfigure={(selected) => onConfigure(source, selected)}
              onRequestDisconnect={onRequestDisconnect}
              onCancelDisconnect={onCancelDisconnect}
              onDisconnect={onDisconnect}
            />
          ))
        )}
      </div>
    </article>
  );
}
