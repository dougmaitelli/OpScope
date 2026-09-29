import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { ProjectGroup } from "./ProjectGroup.tsx";

const project = {
  sourceId: "github",
  sourceName: "GitHub",
  repositoryId: "1",
  repositoryOwner: "team",
  repositoryName: "app",
};
const props = { project, countLabel: "1 workflow", itemCount: 1 };

describe("ProjectGroup", () => {
  it("does not render empty projects", () => {
    const { container } = render(
      <ProjectGroup {...props} itemCount={0}>
        Build
      </ProjectGroup>,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("starts collapsed and lets the user expand and collapse it", async () => {
    const user = userEvent.setup();
    render(
      <ProjectGroup {...props}>
        <button>Build</button>
      </ProjectGroup>,
    );
    expect(screen.getByText("Build")).not.toBeVisible();
    await user.click(screen.getByText("team/app"));
    await waitFor(() => expect(screen.getByText("Build")).toBeVisible());
    await user.click(screen.getByText("team/app"));
    await waitFor(() => expect(screen.getByText("Build")).not.toBeVisible());
  });

  it("supports initial expansion without overriding the user's choice on refresh", async () => {
    const user = userEvent.setup();
    const { rerender } = render(
      <ProjectGroup {...props} defaultExpanded>
        Build
      </ProjectGroup>,
    );
    expect(screen.getByText("Build")).toBeVisible();
    await user.click(screen.getByText("team/app"));
    await waitFor(() => expect(screen.getByText("Build")).not.toBeVisible());
    rerender(
      <ProjectGroup {...props} defaultExpanded countLabel="2 workflows" itemCount={2}>
        Build
      </ProjectGroup>,
    );
    expect(screen.getByText("Build")).not.toBeVisible();
  });
});
