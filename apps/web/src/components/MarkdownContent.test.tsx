import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { MarkdownContent } from "./MarkdownContent";

const baseUrl = "https://github.example/team/project/pull/42";

describe("MarkdownContent", () => {
  it("renders headings, emphasis, task lists, code and GFM tables", () => {
    const { container } = render(
      <MarkdownContent
        baseUrl={baseUrl}
        content={[
          "## Changes",
          "",
          "A **strong** improvement with ~~old text~~.",
          "",
          "- [x] Tested",
          "- [ ] Pending",
          "",
          "```html",
          "<script>example</script>",
          "```",
          "",
          "| Before | After |",
          "| --- | --- |",
          "| slow | fast |",
        ].join("\n")}
      />,
    );
    expect(screen.getByRole("heading", { name: "Changes", level: 2 })).toBeInTheDocument();
    expect(container.querySelector("strong")).toHaveTextContent("strong");
    expect(container.querySelector("del")).toHaveTextContent("old text");
    const checkboxes = screen.getAllByRole("checkbox");
    expect(checkboxes[0]).toBeChecked();
    expect(checkboxes[1]).not.toBeChecked();
    checkboxes.forEach((checkbox) => expect(checkbox).toBeDisabled());
    expect(container.querySelector("pre code")).toHaveTextContent("<script>example</script>");
    expect(container.querySelector("script")).toBeNull();
    expect(screen.getByRole("cell", { name: "fast" })).toBeInTheDocument();
  });

  it("renders safe embedded HTML including collapsible release notes", () => {
    const { container } = render(
      <MarkdownContent
        baseUrl={baseUrl}
        content={
          "<details><summary>Release notes</summary><p>An <strong>important</strong> fix.</p></details><!-- hidden template -->"
        }
      />,
    );
    expect(container.querySelector("details summary")).toHaveTextContent("Release notes");
    expect(container.querySelector("details strong")).toHaveTextContent("important");
    expect(container).not.toHaveTextContent("hidden template");
  });

  it("strips unsafe HTML, handlers, styles and executable URLs", () => {
    const { container } = render(
      <MarkdownContent
        baseUrl={baseUrl}
        content={[
          "<script>alert(1)</script><style>body { display:none }</style>",
          '<iframe src="https://evil.example"></iframe><form>private form</form>',
          '<p onclick="alert(1)" style="position:fixed" class="issue-dialog">Safe text</p>',
          '<a href="javascript:alert(1)">Bad script</a>',
          '<a href="data:text/html,hello">Bad data</a>',
          '<a href="file:///etc/passwd">Bad file</a>',
          '<a href="https://user:password@evil.example">Bad credentials</a>',
          '<a href="https://evil.example/&#9;path">Bad control</a>',
        ].join("\n")}
      />,
    );
    expect(
      container.querySelector("script, style, iframe, form, [onclick], [style], .issue-dialog"),
    ).toBeNull();
    expect(container).not.toHaveTextContent("private form");
    expect(screen.queryAllByRole("link")).toHaveLength(0);
    expect(screen.getByText("Safe text")).toBeInTheDocument();
  });

  it("resolves relative links against the provider and opens links safely", () => {
    render(
      <MarkdownContent
        baseUrl={baseUrl}
        content="[Repository](/team/project) [Sibling](43) [Email](mailto:team@example.com)"
      />,
    );
    expect(screen.getByRole("link", { name: "Repository" })).toHaveAttribute(
      "href",
      "https://github.example/team/project",
    );
    expect(screen.getByRole("link", { name: "Sibling" })).toHaveAttribute(
      "href",
      "https://github.example/team/project/pull/43",
    );
    expect(screen.getByRole("link", { name: "Email" })).toHaveAttribute(
      "href",
      "mailto:team@example.com",
    );
    screen.getAllByRole("link").forEach((link) => {
      expect(link).toHaveAttribute("target", "_blank");
      expect(link).toHaveAttribute("rel", "noopener noreferrer");
    });
  });

  it("shows images as links without loading remote images or nesting anchors", () => {
    const { container } = render(
      <MarkdownContent
        baseUrl={baseUrl}
        content="![Screenshot](https://images.example/screenshot.png) [![Badge](https://images.example/badge.svg)](https://ci.example) <img src='data:image/png;base64,abc' alt='Unsafe' />"
      />,
    );
    expect(container.querySelector("img, a a")).toBeNull();
    expect(screen.getByRole("link", { name: "Image: Screenshot" })).toHaveAttribute(
      "href",
      "https://images.example/screenshot.png",
    );
    expect(screen.getByRole("link", { name: "Image: Badge" })).toHaveAttribute(
      "href",
      "https://ci.example/",
    );
    expect(screen.queryByRole("link", { name: "Image: Unsafe" })).not.toBeInTheDocument();
  });

  it("scopes HTML fragment navigation to each rendered document", () => {
    const content = '<h2 id="notes">Notes</h2><a href="#notes">Jump</a>';
    const { container } = render(
      <>
        <MarkdownContent baseUrl={baseUrl} content={content} />
        <MarkdownContent baseUrl={baseUrl} content={content} />
      </>,
    );
    const headings = container.querySelectorAll("h2");
    expect(headings.item(0).id).not.toBe(headings.item(1).id);
    const firstScroll = vi.fn();
    const secondScroll = vi.fn();
    headings.item(0).scrollIntoView = firstScroll;
    headings.item(1).scrollIntoView = secondScroll;
    fireEvent.click(screen.getAllByRole("link", { name: "Jump" }).at(1)!);
    expect(firstScroll).not.toHaveBeenCalled();
    expect(secondScroll).toHaveBeenCalledWith({ block: "nearest" });
  });
});
