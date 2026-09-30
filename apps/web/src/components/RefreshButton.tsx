export function RefreshButton({
  busy,
  disabled = false,
  onRefresh,
}: {
  busy: boolean;
  disabled?: boolean;
  onRefresh: () => Promise<void>;
}) {
  return (
    <button
      className="secondary-button"
      type="button"
      disabled={disabled || busy}
      aria-busy={busy}
      onClick={() => void onRefresh()}
    >
      {busy ? "Refreshing…" : "Refresh"}
    </button>
  );
}
