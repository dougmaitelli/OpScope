import type { Page } from "@playwright/test";
import { expect } from "./fixtures";

export const screens = [
  { path: "/", title: "Workflows", ready: "Build and test", name: "workflows" },
  {
    path: "/pull-requests",
    title: "Pull requests",
    ready: "Improve build checks",
    name: "pull-requests",
  },
  {
    path: "/issues",
    title: "Issues",
    ready: "Retry failed deployments",
    name: "issues",
  },
  {
    path: "/activity",
    title: "Activity",
    ready: "#128 · Improve deployment reliability",
    name: "activity",
  },
  {
    path: "/settings",
    title: "Settings",
    ready: "Save settings",
    name: "settings",
  },
] as const;

export async function openScreen(page: Page, screen: (typeof screens)[number]) {
  await page.goto(screen.path);
  await expect(
    page.getByRole("heading", { name: screen.title, exact: true, level: 1 }),
  ).toBeVisible();
  await expect(page.getByText(screen.ready).first()).toBeVisible();
  if (screen.name === "workflows") {
    await page.locator(".run-history > summary").click();
    await expect(
      page.getByRole("button", {
        name: "#126 · Cache dependencies between builds",
      }),
    ).toBeVisible();
  }
  if (screen.name === "settings")
    await expect(page.getByRole("checkbox").first()).toBeEnabled();
}
