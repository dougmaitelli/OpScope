import { useId } from "react";
import type { ReactNode } from "react";
import "./InventorySummary.css";

type SummaryTone = "idle" | "passing" | "running" | "failing";

interface SummaryStat {
  label: string;
  value: number | string;
  tone?: SummaryTone;
  onSelect?: () => void;
  actionLabel?: string;
}

interface InventorySummaryProps {
  heading: string;
  description: ReactNode;
  tone?: SummaryTone;
  busy?: boolean;
  stats: SummaryStat[];
}

export function InventorySummary({
  heading,
  description,
  tone = "idle",
  busy = false,
  stats,
}: InventorySummaryProps) {
  const headingId = useId();
  const icon = tone === "failing" ? "!" : tone === "running" ? "↻" : tone === "passing" ? "✓" : "·";

  return (
    <section className="summary-bar" aria-labelledby={headingId} aria-busy={busy}>
      <div className="summary-heading">
        <span className={`summary-icon summary-icon-${tone}`} aria-hidden="true">
          {icon}
        </span>
        <div>
          <h2 id={headingId}>{heading}</h2>
          <p>{description}</p>
        </div>
      </div>
      <dl className="summary-stats">
        {stats.map((stat) => (
          <div key={stat.label}>
            <dt>{stat.label}</dt>
            <dd>
              {stat.onSelect ? (
                <button
                  className={`summary-stat-link summary-stat-link-${stat.tone ?? "idle"}`}
                  type="button"
                  aria-label={stat.actionLabel}
                  onClick={stat.onSelect}
                >
                  {stat.value}
                </button>
              ) : (
                stat.value
              )}
            </dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
