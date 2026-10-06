import { useEffect, useEffectEvent, useId, useLayoutEffect, useRef, useState } from "react";
import { useApplicationClient } from "../api/application-client.tsx";
import type {
  ActionOptions,
  ActionTarget,
  AvailableAction,
  ExecuteActionResponse,
} from "../generated/contracts.ts";
import { requestErrorSentence } from "../shared/errors.ts";
import "./SourceActions.css";

export function SourceActions({
  sourceId,
  repositoryId,
  target,
  onAccepted,
  onOptionsLoaded,
}: {
  sourceId: string;
  repositoryId: string;
  target: ActionTarget;
  onOptionsLoaded?: (options: ActionOptions) => void;
  onAccepted?: (response: ExecuteActionResponse) => void;
}) {
  const client = useApplicationClient();
  const [options, setOptions] = useState<ActionOptions | null>(null);
  const [selected, setSelected] = useState<AvailableAction | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [accepted, setAccepted] = useState(false);
  const submitting = useRef(false);
  const trigger = useRef<HTMLButtonElement | null>(null);
  const confirmation = useRef<HTMLDivElement | null>(null);
  const confirmButton = useRef<HTMLButtonElement | null>(null);
  const targetKey = JSON.stringify(target);
  const descriptionId = useId();
  const confirmationId = useId();
  const disabledReasons = [
    ...new Set(
      (options?.actions ?? []).flatMap((option) =>
        option.disabledReason ? [option.disabledReason] : [],
      ),
    ),
  ];

  const optionsLoaded = useEffectEvent((response: ActionOptions) => onOptionsLoaded?.(response));

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
        if (active) {
          optionsLoaded(response);
          setOptions(response);
        }
      })
      .catch((failure: unknown) => {
        if (active) setError(requestErrorSentence(failure));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [client, sourceId, repositoryId, targetKey]);

  function dismissConfirmation(restoreFocus = true) {
    if (submitting.current) return;
    setSelected(null);
    if (restoreFocus) trigger.current?.focus();
  }

  useLayoutEffect(() => {
    if (!selected) return;
    const popover = confirmation.current;
    const anchor = trigger.current;
    if (!popover || !anchor) return;

    // The top layer keeps confirmations visible inside scrollable dialogs.
    popover.showPopover?.();
    const position = () => {
      const rect = anchor.getBoundingClientRect();
      const width = popover.offsetWidth;
      const height = popover.offsetHeight;
      const below = rect.bottom + 8;
      const top = below + height <= window.innerHeight - 12 ? below : rect.top - height - 8;
      popover.style.left = `${Math.max(12, Math.min(rect.left, window.innerWidth - width - 12))}px`;
      popover.style.top = `${Math.max(12, Math.min(top, window.innerHeight - height - 12))}px`;
    };
    position();
    confirmButton.current?.focus();

    const outside = (event: Event) => {
      if (
        !submitting.current &&
        event.target instanceof Node &&
        !popover.contains(event.target) &&
        !anchor.contains(event.target)
      ) {
        setSelected(null);
      }
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      // Dismiss this confirmation before Escape can close the parent dialog.
      event.preventDefault();
      event.stopPropagation();
      if (!submitting.current) {
        setSelected(null);
        anchor.focus();
      }
    };
    document.addEventListener("pointerdown", outside);
    document.addEventListener("focusin", outside);
    document.addEventListener("keydown", escape, true);
    window.addEventListener("resize", position);
    document.addEventListener("scroll", position, true);
    return () => {
      document.removeEventListener("pointerdown", outside);
      document.removeEventListener("focusin", outside);
      document.removeEventListener("keydown", escape, true);
      window.removeEventListener("resize", position);
      document.removeEventListener("scroll", position, true);
    };
  }, [selected]);

  async function execute() {
    if (!selected || !options || submitting.current) return;
    submitting.current = true;
    setBusy(true);
    setError(null);
    try {
      const response = await client.executeAction({
        sourceId,
        repositoryId,
        target,
        action: selected.action,
        revision: options.revision,
      });
      setAccepted(true);
      onAccepted?.(response);
    } catch (failure) {
      setError(requestErrorSentence(failure));
      // Require a fresh availability check before another manual attempt.
      setOptions(null);
    } finally {
      setSelected(null);
      setBusy(false);
      submitting.current = false;
      trigger.current?.focus();
    }
  }

  return (
    <section className="source-actions" aria-label="Provider actions" aria-busy={loading || busy}>
      {loading ? <span>Checking available actions…</span> : null}
      {!loading && !accepted ? (
        <>
          <div className="source-actions-buttons">
            {options?.actions.map((option) => (
              <button
                key={option.action}
                type="button"
                className="secondary-button compact-button source-action-button"
                data-action={option.action}
                disabled={busy || option.disabledReason !== null}
                title={option.disabledReason ?? option.confirmation}
                aria-haspopup="dialog"
                aria-expanded={selected?.action === option.action}
                aria-controls={selected?.action === option.action ? confirmationId : undefined}
                aria-describedby={
                  option.disabledReason
                    ? `${descriptionId}-${disabledReasons.indexOf(option.disabledReason)}`
                    : undefined
                }
                onClick={(event) => {
                  if (selected?.action === option.action) {
                    dismissConfirmation();
                  } else {
                    trigger.current = event.currentTarget;
                    setSelected(option);
                  }
                }}
              >
                {option.label}
              </button>
            ))}
            {options?.actions.length === 0 ? (
              <span>No supported actions for this resource.</span>
            ) : null}
          </div>
          {disabledReasons.length > 0 ? (
            <div className="source-actions-notes">
              {disabledReasons.map((reason, index) => (
                <p className="source-actions-note" id={`${descriptionId}-${index}`} key={reason}>
                  <strong>
                    {options?.actions
                      .filter((option) => option.disabledReason === reason)
                      .map((option) => option.label)
                      .join(", ")}
                    :{" "}
                  </strong>{" "}
                  <span>{reason}</span>
                </p>
              ))}
            </div>
          ) : null}
        </>
      ) : null}
      {selected ? (
        <div
          ref={confirmation}
          id={confirmationId}
          className="source-actions-confirmation"
          popover="manual"
          role="dialog"
          aria-label={`Confirm ${selected.label}`}
          aria-describedby={`${confirmationId}-description`}
          aria-busy={busy}
        >
          <p id={`${confirmationId}-description`}>{selected.confirmation}</p>
          <div className="source-actions-confirmation-buttons">
            <button
              ref={confirmButton}
              type="button"
              className="secondary-button compact-button source-action-button"
              data-action={selected.action}
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
              className="danger-button compact-button"
              disabled={busy}
              onClick={() => dismissConfirmation()}
            >
              Cancel
            </button>
          </div>
        </div>
      ) : null}
      {error ? <p role="alert">{error}</p> : null}
      {accepted ? (
        <p role="status">
          Request accepted by the provider. Completion may take a moment; subsequent synchronization
          will pick up the result.
        </p>
      ) : null}
    </section>
  );
}
