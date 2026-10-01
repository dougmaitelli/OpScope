import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
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
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it.each(["Cancel", "Escape", "outside"])(
    "keeps the action row mounted and dismisses confirmation with %s",
    async (dismissal) => {
      const user = userEvent.setup();
      render(
        <>
          <SourceActions {...props} />
          <button>Outside</button>
        </>,
      );
      const trigger = await screen.findByRole("button", { name: "Update branch" });
      const row = trigger.parentElement;
      await user.click(trigger);
      expect(screen.getByRole("button", { name: "Update branch" })).toBe(trigger);
      expect(trigger.parentElement).toBe(row);
      expect(trigger).toHaveAttribute("aria-expanded", "true");
      const popover = screen.getByRole("dialog", { name: "Confirm Update branch" });
      expect(popover).toHaveAttribute("popover", "manual");
      expect(popover).toHaveAccessibleDescription("Merge the target branch into this PR branch?");
      expect(row).not.toContainElement(popover);
      expect(screen.getByRole("button", { name: "Confirm action" })).toHaveFocus();

      if (dismissal === "Escape") await user.keyboard("{Escape}");
      else
        await user.click(
          screen.getByRole("button", { name: dismissal === "Cancel" ? "Cancel" : "Outside" }),
        );

      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(trigger).toHaveAttribute("aria-expanded", "false");
      expect(client.executeAction).not.toHaveBeenCalled();
      expect(
        dismissal === "outside" ? screen.getByRole("button", { name: "Outside" }) : trigger,
      ).toHaveFocus();
    },
  );

  it("dismisses confirmation when keyboard focus leaves it", async () => {
    const user = userEvent.setup();
    render(
      <>
        <SourceActions {...props} />
        <button>Outside</button>
      </>,
    );
    await user.click(await screen.findByRole("button", { name: "Update branch" }));
    await user.tab();
    expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "Outside" })).toHaveFocus();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("anchors confirmation within viewport edges and repositions on scroll", async () => {
    vi.stubGlobal("innerWidth", 400);
    vi.stubGlobal("innerHeight", 300);
    vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(340);
    vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(100);
    const user = userEvent.setup();
    render(<SourceActions {...props} />);
    const trigger = await screen.findByRole("button", { name: "Update branch" });
    const rect = vi.spyOn(trigger, "getBoundingClientRect").mockReturnValue({
      left: 350,
      top: 200,
      bottom: 228,
      right: 390,
      width: 40,
      height: 28,
      x: 350,
      y: 200,
      toJSON: () => ({}),
    });
    await user.click(trigger);
    const popover = screen.getByRole("dialog");
    expect(popover).toHaveStyle({ left: "48px", top: "92px" });
    rect.mockReturnValue({
      left: 20,
      top: 40,
      bottom: 68,
      right: 60,
      width: 40,
      height: 28,
      x: 20,
      y: 40,
      toJSON: () => ({}),
    });
    fireEvent.scroll(document);
    expect(popover).toHaveStyle({ left: "20px", top: "76px" });
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
    const confirm = screen.getByRole("button", { name: "Confirm action" });
    const cancel = screen.getByRole("button", { name: "Cancel" });
    expect(confirm.parentElement).toHaveClass("source-actions-confirmation-buttons");
    expect(confirm.nextElementSibling).toBe(cancel);
    expect(cancel).toHaveClass("danger-button", "compact-button");
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
    await user.keyboard("{Escape}");
    await user.click(document.body);
    expect(screen.getByRole("dialog", { name: "Confirm Update branch" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();
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

  it("keeps buttons together and groups shared disabled explanations outside the button row", async () => {
    const reason = "GitHub is still checking mergeability.";
    client.actionOptions.mockResolvedValue({
      revision: "head-sha",
      actions: [
        { ...options.actions[0], disabledReason: reason },
        {
          ...options.actions[0],
          action: "mergeChangeRequest",
          label: "Merge PR",
          disabledReason: reason,
        },
        {
          ...options.actions[0],
          action: "dependabotRebase",
          label: "Rebase",
          disabledReason: null,
        },
      ],
    });
    render(<SourceActions {...props} />);
    const update = await screen.findByRole("button", { name: "Update branch" });
    const merge = screen.getByRole("button", { name: "Merge PR" });
    const rebase = screen.getByRole("button", { name: "Rebase" });
    expect(update.parentElement).toHaveClass("source-actions-buttons");
    expect(update.nextElementSibling).toBe(merge);
    expect(merge.nextElementSibling).toBe(rebase);
    expect(screen.getAllByText(reason)).toHaveLength(1);
    expect(update.parentElement).not.toContainElement(screen.getByText(reason));
    expect(update).toHaveAccessibleDescription(`Update branch, Merge PR: ${reason}`);
    expect(merge).toHaveAccessibleDescription(`Update branch, Merge PR: ${reason}`);
    expect(rebase).toBeEnabled();
  });

  it("reports a failed mutation and checks actions again when reopened", async () => {
    client.executeAction.mockRejectedValue({
      message: "The provider did not confirm the result. Check before retrying.",
    });
    const user = userEvent.setup();
    const { unmount } = render(<SourceActions {...props} />);
    await user.click(await screen.findByRole("button", { name: "Update branch" }));
    await user.click(screen.getByRole("button", { name: "Confirm action" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Check before retrying");
    expect(screen.queryByRole("button", { name: "Update branch" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Reload actions" })).not.toBeInTheDocument();
    unmount();
    render(<SourceActions {...props} />);
    expect(await screen.findByRole("button", { name: "Update branch" })).toBeEnabled();
    expect(client.actionOptions).toHaveBeenCalledTimes(2);
    expect(client.executeAction).toHaveBeenCalledTimes(1);
  });

  it("explains when no actions are supported", async () => {
    client.actionOptions.mockResolvedValue({ actions: [], revision: null });
    render(<SourceActions {...props} />);
    expect(await screen.findByText("No supported actions for this resource.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Reload actions" })).not.toBeInTheDocument();
    expect(client.executeAction).not.toHaveBeenCalled();
  });

  it.each([
    "provider credential lacks required permission",
    "Provider credential lacks required permission.",
  ])("formats provider errors as complete sentences: %s", async (message) => {
    client.executeAction.mockRejectedValue({ message });
    const user = userEvent.setup();
    render(<SourceActions {...props} />);
    await user.click(await screen.findByRole("button", { name: "Update branch" }));
    await user.click(screen.getByRole("button", { name: "Confirm action" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      /^Provider credential lacks required permission\.$/,
    );
  });
});
