import { formatRelativeDate } from "../shared/time.ts";

const absoluteTimeFormatter = new Intl.DateTimeFormat(undefined, {
  year: "numeric",
  month: "short",
  day: "numeric",
  hour: "numeric",
  minute: "2-digit",
  second: "2-digit",
  timeZoneName: "short",
});

interface TimeDisplayProps {
  /** ISO datetime or Unix timestamp in seconds. */
  dateTime: string | number;
  duration?: string | null;
  elapsed?: boolean;
}

export function TimeDisplay({ dateTime, duration, elapsed = false }: TimeDisplayProps) {
  const date = new Date(typeof dateTime === "number" ? dateTime * 1000 : dateTime);
  const valid = !Number.isNaN(date.getTime());
  const absolute = valid ? absoluteTimeFormatter.format(date) : undefined;
  const iso = valid ? date.toISOString() : undefined;

  return (
    <span title={absolute}>
      {duration ? `${duration}${elapsed ? " elapsed" : ""} · ` : ""}
      <time dateTime={iso} aria-label={absolute}>
        {iso ? formatRelativeDate(iso) : "Unknown time"}
      </time>
    </span>
  );
}
