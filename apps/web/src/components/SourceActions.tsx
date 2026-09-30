import { useEffect, useRef, useState } from "react";
import { useApplicationClient } from "../api/application-client.tsx";
import type { ActionOptions, ActionTarget, AvailableAction } from "../generated/contracts.ts";
import { requestErrorMessage } from "../shared/errors.ts";
import "./SourceActions.css";

export function SourceActions({
  sourceId,
  repositoryId,
  target,
  onAccepted,
}: {
  sourceId: string;
  repositoryId: string;
  target: ActionTarget;
  onAccepted?: () => void;
}) {
  const client = useApplicationClient();
  const [options, setOptions] = useState<ActionOptions | null>(null);
  const [selected, setSelected] = useState<AvailableAction | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [accepted, setAccepted] = useState(false);
  const [reload, setReload] = useState(0);
  const submitting = useRef(false);
  const targetKey = JSON.stringify(target);

  useEffect(() => {
    let active = true;
    setLoading(true);
    setOptions(null);
    setSelected(null);
    setError(null);
    setAccepted(false);
    void client
      .actionOptions({ sourceId, repositoryId, target: JSON.parse(targetKey) as ActionTarget })
      .then((response) => {
        if (active) setOptions(response);
      })
      .catch((failure: unknown) => {
        if (active) setError(requestErrorMessage(failure));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [client, sourceId, repositoryId, targetKey, reload]);

  async function execute() {
    if (!selected || !options || submitting.current) return;
    submitting.current = true;
    setBusy(true);
    setError(null);
    try {
      await client.executeAction({
        sourceId,
        repositoryId,
        target,
        action: selected.action,
        revision: options.revision,
      });
      setAccepted(true);
      onAccepted?.();
    } catch (failure) {
      setError(requestErrorMessage(failure));
      // Require a fresh availability check before another manual attempt.
      setOptions(null);
    } finally {
      setSelected(null);
      setBusy(false);
      submitting.current = false;
    }
  }

  return (
    <section className="source-actions" aria-label="Provider actions" aria-busy={loading || busy}>
      {loading ? <span>Checking available actions…</span> : null}
      {!loading && !selected && !accepted ? (
        <div className="source-actions-buttons">
          {options?.actions.map((option) => (
            <span key={option.action}>
              <button
                type="button"
                className="secondary-button compact-button"
                disabled={busy || option.disabledReason !== null}
                title={option.disabledReason ?? option.confirmation}
                onClick={() => setSelected(option)}
              >
                {option.label}
              </button>
              {option.disabledReason ? <small>{option.disabledReason}</small> : null}
            </span>
          ))}
          {options?.actions.length === 0 ? (
            <span>No supported actions for this resource.</span>
          ) : null}
        </div>
      ) : null}
      {selected ? (
        <div className="source-actions-confirmation">
          <p>{selected.confirmation}</p>
          <button
            type="button"
            className="primary-button compact-button"
            disabled={busy}
            aria-busy={busy}
            onClick={() => {
              void execute();
            }}
          >
            {busy ? "Sending…" : "Confirm action"}
          </button>
          <button
            type="button"
            className="secondary-button compact-button"
            disabled={busy}
            onClick={() => setSelected(null)}
          >
            Cancel
          </button>
        </div>
      ) : null}
      {error ? (
        <p role="alert">
          {error} If a connection was interrupted while sending an action, check the provider before
          retrying.
        </p>
      ) : null}
      {accepted ? (
        <p role="status">
          Request accepted by the provider. Completion may take a moment; subsequent synchronization
          will pick up the result.
        </p>
      ) : null}
      {!loading && !busy && !selected ? (
        <button
          type="button"
          className="secondary-button compact-button"
          onClick={() => setReload((value) => value + 1)}
        >
          Reload actions
        </button>
      ) : null}
    </section>
  );
}
