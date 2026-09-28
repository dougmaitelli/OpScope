import type { ReactNode } from "react";
import "./StatusPill.css";

export function StatusPill({
  tone = "neutral",
  children,
  className = "",
}: {
  tone?: string;
  children: ReactNode;
  className?: string;
}) {
  return <span className={`status-pill status-pill-${tone} ${className}`}>{children}</span>;
}
