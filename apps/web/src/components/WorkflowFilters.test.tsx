import { useState } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { WorkflowFilters, type WorkflowStatusFilter } from "./WorkflowFilters.tsx";

function Filters() {
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState<WorkflowStatusFilter>("all");
  return (
    <WorkflowFilters
      query={query}
      status={status}
      onQueryChange={setQuery}
      onStatusChange={setStatus}
      onClear={() => {
        setQuery("");
        setStatus("all");
      }}
    />
  );
}

describe("WorkflowFilters", () => {
  it("disables clearing when no filters are active", () => {
    render(<Filters />);
    expect(screen.getByRole("button", { name: "Clear" })).toBeDisabled();
  });

  it("accepts a search and clears both search and status", async () => {
    const user = userEvent.setup();
    render(<Filters />);
    const search = screen.getByRole("searchbox", { name: "Search" });
    const status = screen.getByRole("combobox", { name: "Status" });
    await user.type(search, "build");
    await user.selectOptions(status, "failing");
    expect(search).toHaveValue("build");
    expect(status).toHaveValue("failing");
    await user.click(screen.getByRole("button", { name: "Clear" }));
    expect(search).toHaveValue("");
    expect(status).toHaveValue("all");
    expect(screen.getByRole("button", { name: "Clear" })).toBeDisabled();
  });

  it("allows a running-only filter without a search", async () => {
    const user = userEvent.setup();
    render(<Filters />);
    await user.selectOptions(screen.getByRole("combobox", { name: "Status" }), "running");
    expect(screen.getByRole("combobox", { name: "Status" })).toHaveValue("running");
    expect(screen.getByRole("button", { name: "Clear" })).toBeEnabled();
  });
});
