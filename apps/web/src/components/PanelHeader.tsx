import type { ReactNode } from "react";

interface PanelHeaderProps {
  label: string;
  title: string;
  metadata?: ReactNode;
}

export function PanelHeader({ label, title, metadata }: PanelHeaderProps) {
  return (
    <div className="panel-header">
      <div>
        <p className="section-label">{label}</p>
        <h2>{title}</h2>
      </div>
      {metadata}
    </div>
  );
}
