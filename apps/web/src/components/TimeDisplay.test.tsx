import { render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TimeDisplay } from "./TimeDisplay.tsx";

describe("time display", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-09-30T19:04:00Z"));
  });
  afterEach(() => vi.useRealTimers());

  it("shows relative time and exposes the complete local timestamp on hover", () => {
    render(<TimeDisplay dateTime="2026-09-30T19:00:00Z" />);
    const time = screen.getByText("4m ago");
    expect(time).toHaveAttribute("datetime", "2026-09-30T19:00:00.000Z");
    const title = time.parentElement?.getAttribute("title");
    expect(title).toBeTruthy();
    expect(title).toContain("2026");
    expect(title).toContain(":00");
    expect(time).toHaveAccessibleName(title ?? "");
  });

  it("includes an optional duration and elapsed label", () => {
    const { rerender } = render(<TimeDisplay dateTime="2026-09-30T19:00:00Z" duration="15m 5s" />);
    const time = screen.getByText("4m ago");
    expect(time.parentElement).toHaveTextContent("15m 5s · 4m ago");
    rerender(<TimeDisplay dateTime="2026-09-30T19:00:00Z" duration="15m 5s" elapsed />);
    expect(time.parentElement).toHaveTextContent("15m 5s elapsed · 4m ago");
    rerender(<TimeDisplay dateTime="2026-09-30T19:00:00Z" duration={null} />);
    expect(time.parentElement).toHaveTextContent(/^4m ago$/);
  });

  it("handles Unix seconds without confusing them with milliseconds", () => {
    render(<TimeDisplay dateTime={Date.parse("2026-09-30T19:00:00Z") / 1000} />);
    expect(screen.getByText("4m ago")).toHaveAttribute("datetime", "2026-09-30T19:00:00.000Z");
  });

  it("shows an unknown time without an invalid tooltip or datetime", () => {
    render(<TimeDisplay dateTime="invalid" />);
    const time = screen.getByText("Unknown time");
    expect(time).not.toHaveAttribute("datetime");
    expect(time.parentElement).not.toHaveAttribute("title");
  });
});
