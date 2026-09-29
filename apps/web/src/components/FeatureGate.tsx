import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import { useMonitoringSettings } from "../api/monitoring-settings.ts";
import { EmptyState } from "./EmptyState.tsx";
import "./FeatureGate.css";

export function FeatureGate({
  feature,
  label,
  children,
}: {
  feature: "pullRequestsEnabled" | "issuesEnabled";
  label: string;
  children: ReactNode;
}) {
  const { settings, error, reload } = useMonitoringSettings();
  if (!settings)
    return (
      <section className="panel feature-gate">
        <EmptyState message={error ?? "Loading settings…"} error={Boolean(error)} />
        {error ? (
          <button className="secondary-button" onClick={() => void reload()}>
            Retry
          </button>
        ) : null}
      </section>
    );
  if (!settings[feature])
    return (
      <section className="panel feature-gate">
        <EmptyState message={`${label} monitoring is disabled.`} />
        <Link className="secondary-button" to="/settings">
          Manage features in Settings
        </Link>
      </section>
    );
  return children;
}
