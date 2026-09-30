import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SourceActions } from "./SourceActions";

const client = vi.hoisted(() => ({ actionOptions: vi.fn(), executeAction: vi.fn() }));
vi.mock("../api/application-client.tsx", () => ({ useApplicationClient: () => client }));
const props = {
  sourceId: "connection",
  repositoryId: "repo",
  target: { type: "changeRequest" as const, number: 7 },
};
const options = {
  revision: "head-sha",
  actions: [
    {
      action: "updateBranch",
      label: "Update branch",
      confirmation: "Merge the target branch into this PR branch?",
      disabledReason: null,
    },
  ],
};

describe("provider actions", () => {
  it("renders a merge action generically and submits only after explicit confirmation", async () => {
    client.actionOptions.mockResolvedValue({
      revision: "head-sha",
      actions: [
        {
          action: "mergeChangeRequest",
          label: "Merge PR",
          confirmation: "Merge this PR into main? This may trigger deployments.",
          disabledReason: null,
        },
      ],
    });
    const user = userEvent.setup();
    render(<SourceActions {...props} />);
    await user.click(await screen.findByRole("button", { name: "Merge PR" }));
    expect(
      screen.getByText("Merge this PR into main? This may trigger deployments."),
    ).toBeInTheDocument();
    expect(client.executeAction).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Confirm action" }));
    expect(client.executeAction).toHaveBeenCalledExactlyOnceWith({
      ...props,
      action: "mergeChangeRequest",
      revision: "head-sha",
    });
  });
  beforeEach(() => {
    vi.resetAllMocks();
    client.actionOptions.mockResolvedValue(options);
    client.executeAction.mockResolvedValue({ accepted: true });
  });

  it("requires confirmation, sends the provider revision, and prevents duplicate submissions", async () => {
    let resolve!: (result: { accepted: boolean }) => void;
    client.executeAction.mockReturnValue(
      new Promise((done) => {
        resolve = done;
      }),
    );
    const onAccepted = vi.fn();
    const user = userEvent.setup();
    render(<SourceActions {...props} onAccepted={onAccepted} />);
    await user.click(await screen.findByRole("button", { name: "Update branch" }));
    expect(client.executeAction).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(client.executeAction).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Update branch" }));
    await user.dblClick(screen.getByRole("button", { name: "Confirm action" }));
    expect(client.executeAction).toHaveBeenCalledExactlyOnceWith({
      ...props,
      action: "updateBranch",
      revision: "head-sha",
    });
    expect(screen.getByRole("button", { name: "Sending…" })).toBeDisabled();
    resolve({ accepted: true });
    expect(await screen.findByRole("status")).toHaveTextContent("Request accepted");
    expect(onAccepted).toHaveBeenCalledOnce();
  });

  it("shows why an action is disabled without a busy indicator", async () => {
    client.actionOptions.mockResolvedValue({
      ...options,
      actions: [{ ...options.actions[0], disabledReason: "Resolve conflicts first." }],
    });
    render(<SourceActions {...props} />);
    const button = await screen.findByRole("button", { name: "Update branch" });
    expect(button).toBeDisabled();
    expect(button).not.toHaveAttribute("aria-busy", "true");
    expect(screen.getByText("Resolve conflicts first.")).toBeInTheDocument();
  });

  it("reports a failed mutation and requires reloading before a retry", async () => {
    client.executeAction.mockRejectedValue({
      message: "The provider did not confirm the result. Check before retrying.",
    });
    const user = userEvent.setup();
    render(<SourceActions {...props} />);
    await user.click(await screen.findByRole("button", { name: "Update branch" }));
    await user.click(screen.getByRole("button", { name: "Confirm action" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Check before retrying");
    expect(screen.queryByRole("button", { name: "Update branch" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Reload actions" }));
    await waitFor(() => expect(client.actionOptions).toHaveBeenCalledTimes(2));
    expect(await screen.findByRole("button", { name: "Update branch" })).toBeEnabled();
    expect(client.executeAction).toHaveBeenCalledTimes(1);
  });

  it("explains when no actions are supported", async () => {
    client.actionOptions.mockResolvedValue({ actions: [], revision: null });
    render(<SourceActions {...props} />);
    expect(await screen.findByText("No supported actions for this resource.")).toBeInTheDocument();
    expect(client.executeAction).not.toHaveBeenCalled();
  });
});
