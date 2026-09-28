import type { ComponentPropsWithoutRef, ReactNode } from "react";
import "./DataRow.css";

type DataRowProps =
  | ({ as?: "button"; surface?: boolean } & ComponentPropsWithoutRef<"button">)
  | ({ as: "div"; surface?: boolean } & ComponentPropsWithoutRef<"div">);

// Use the button variant for a single action; use div when the row contains links/buttons.
export function DataRow({ className = "", surface = true, ...props }: DataRowProps) {
  const classes = `data-row${surface ? " data-row-surface" : ""} ${className}`;
  if (props.as === "div") {
    const { as: Tag, ...attributes } = props;
    return <Tag {...attributes} className={classes} />;
  }
  const { as: Tag = "button", type = "button", ...attributes } = props;
  return <Tag {...attributes} type={type} className={classes} />;
}

export function DataRowGroup({ className = "", ...props }: ComponentPropsWithoutRef<"article">) {
  return <article {...props} className={`data-row-surface ${className}`} />;
}

export function DataRowIdentity({
  title,
  metadata,
  className = "",
  metadataClassName = "",
  titleAs: Title = "span",
}: {
  title: ReactNode;
  metadata?: ReactNode;
  className?: string;
  metadataClassName?: string;
  titleAs?: "span" | "h4";
}) {
  const Container = Title === "h4" ? "div" : "span";
  return (
    <Container className={`data-row-identity ${className}`}>
      <Title className="data-row-title">{title}</Title>
      {metadata != null ? (
        <DataRowMeta className={metadataClassName}>{metadata}</DataRowMeta>
      ) : null}
    </Container>
  );
}

export function DataRowMeta({ className = "", ...props }: ComponentPropsWithoutRef<"span">) {
  return <span {...props} className={`data-row-meta ${className}`} />;
}

export function DataRowHeader({ className = "", ...props }: ComponentPropsWithoutRef<"div">) {
  return <div aria-hidden="true" {...props} className={`data-row-header ${className}`} />;
}

export function RowChevron({ className = "" }: { className?: string }) {
  return (
    <span className={`data-row-chevron ${className}`} aria-hidden="true">
      ›
    </span>
  );
}
