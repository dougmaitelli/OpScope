import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ApplicationClientProvider } from "../../api/application-client.tsx";
import type { SourceSummary } from "../../generated/contracts.ts";
import { ConnectionDialog } from "./ConnectionDialog.tsx";

const gitlab: SourceSummary = {
  id: "gitlab",
  name: "GitLab",
  abbreviation: "GL",
  description: "GitLab pipelines",
  capabilities: ["workflows"],
  connections: [],
  credential: { label: "Personal access token", placeholder: "glpat-…", help: "read_api" },
  connectionFields: [
    {
      key: "serverUrl",
      inputType: "url",
      label: "Server URL",
      placeholder: "https://gitlab.com",
      defaultValue: "https://gitlab.com",
      help: "Instance origin",
    },
  ],
};
const bitbucket: SourceSummary = {
  ...gitlab,
  id: "bitbucket",
  name: "Bitbucket Cloud",
  abbreviation: "BB",
  credential: { label: "API token", placeholder: "Token", help: "Scoped API token" },
  connectionFields: [
    {
      key: "workspace",
      inputType: "text",
      label: "Workspace",
      placeholder: "team",
      defaultValue: "",
      help: "Workspace slug",
    },
    {
      key: "email",
      inputType: "email",
      label: "Atlassian account email",
      placeholder: "you@example.com",
      defaultValue: "",
      help: "Token owner",
    },
  ],
};

describe("ConnectionDialog provider configuration", () => {
  it("renders source-declared field types and clears secrets when changing providers", async () => {
    const user = userEvent.setup();
    render(
      <ApplicationClientProvider>
        <ConnectionDialog sources={[gitlab, bitbucket]} onSaved={vi.fn()} onClose={vi.fn()} />
      </ApplicationClientProvider>,
    );
    await user.selectOptions(screen.getByLabelText("Source"), "gitlab");
    expect(screen.getByLabelText("Server URL")).toHaveAttribute("type", "url");
    expect(screen.getByLabelText("Server URL")).toHaveValue("https://gitlab.com");
    await user.type(screen.getByLabelText("Personal access token"), "do-not-reuse");
    await user.selectOptions(screen.getByLabelText("Source"), "bitbucket");
    expect(screen.queryByLabelText("Server URL")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Workspace")).toHaveAttribute("type", "text");
    expect(screen.getByLabelText("Atlassian account email")).toHaveAttribute("type", "email");
    expect(screen.getByLabelText("API token")).toHaveValue("");
  });

  it("submits workspace and email through the generic connection client", async () => {
    const user = userEvent.setup();
    const fetch = vi
      .fn<typeof globalThis.fetch>()
      .mockResolvedValue(Response.json({ id: "connection-1", label: "team" }));
    vi.stubGlobal("fetch", fetch);
    const onSaved = vi.fn();
    render(
      <ApplicationClientProvider>
        <ConnectionDialog sources={[gitlab, bitbucket]} onSaved={onSaved} onClose={vi.fn()} />
      </ApplicationClientProvider>,
    );
    await user.selectOptions(screen.getByLabelText("Source"), "bitbucket");
    await user.type(screen.getByLabelText("Workspace"), "team");
    await user.type(screen.getByLabelText("Atlassian account email"), "you@example.com");
    await user.type(screen.getByLabelText("API token"), "test-token");
    await user.click(screen.getByRole("button", { name: "Add connection" }));
    await waitFor(() => expect(onSaved).toHaveBeenCalledOnce());
    expect(fetch).toHaveBeenCalledWith(
      "/api/connections",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          sourceId: "bitbucket",
          connectionId: null,
          configuration: { workspace: "team", email: "you@example.com" },
          credential: "test-token",
        }),
      }),
    );
    expect(screen.getByLabelText("API token")).toHaveValue("");
  });
});
