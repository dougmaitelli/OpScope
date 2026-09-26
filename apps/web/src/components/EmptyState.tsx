import "./EmptyState.css";

export function EmptyState({ message, error = false }: { message: string; error?: boolean }) {
  return <p className={`empty-state${error ? " empty-state-error" : ""}`}>{message}</p>;
}
