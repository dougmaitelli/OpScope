import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { FeatureSettings } from "./FeatureSettings.tsx";

describe("FeatureSettings", () => {
  it("reports changes to each feature independently", async () => {
    const user = userEvent.setup();
    const onPullRequestsChanged = vi.fn();
    const onIssuesChanged = vi.fn();
    render(
      <FeatureSettings
        pullRequestsEnabled
        issuesEnabled={false}
        disabled={false}
        onPullRequestsChanged={onPullRequestsChanged}
        onIssuesChanged={onIssuesChanged}
      />,
    );
    const prs = screen.getByRole("checkbox", { name: /Pull request monitoring/ });
    const issues = screen.getByRole("checkbox", { name: /Issue monitoring/ });
    expect(prs).toBeChecked();
    expect(issues).not.toBeChecked();
    await user.click(prs);
    expect(onPullRequestsChanged).toHaveBeenCalledExactlyOnceWith(false);
    expect(onIssuesChanged).not.toHaveBeenCalled();
    await user.click(issues);
    expect(onIssuesChanged).toHaveBeenCalledExactlyOnceWith(true);
  });

  it("prevents changes while disabled", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(
      <FeatureSettings
        pullRequestsEnabled
        issuesEnabled
        disabled
        onPullRequestsChanged={onChange}
        onIssuesChanged={onChange}
      />,
    );
    for (const checkbox of screen.getAllByRole("checkbox")) {
      expect(checkbox).toBeDisabled();
      await user.click(checkbox);
    }
    expect(onChange).not.toHaveBeenCalled();
  });
});
