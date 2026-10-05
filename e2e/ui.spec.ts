import {
  test,
  expect,
  prepareVisualScreenshot as prepareScreenshot,
} from "./fixtures";
import { screens, openScreen } from "./screens";
import {
  applicationVersion,
  httpRoutes,
} from "../apps/web/src/generated/contracts";

test("sidebar displays the current application version", async ({ page }) => {
  await openScreen(page, screens[0]);
  await expect(page.locator(".sidebar-version > span")).toHaveText(
    `Version ${applicationVersion}`,
  );
});

for (const screen of screens) {
  test(`${screen.title} layout`, async ({ page }) => {
    await openScreen(page, screen);
    await prepareScreenshot(page);
    await expect(page).toHaveScreenshot(`${screen.name}.png`, {
      fullPage: true,
    });
  });
}

test("workflow filtering, URL persistence, refresh and logs", async ({
  page,
  api,
}) => {
  await openScreen(page, screens[0]);
  await page.getByRole("button", { name: "Show 1 failing workflow" }).click();
  await expect(page).toHaveURL(/status=failing/);
  await expect(page.getByRole("heading", { name: "acme/web" })).toHaveCount(0);
  await page.reload();
  await expect(page.getByRole("combobox", { name: "Status" })).toHaveValue(
    "failing",
  );
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect
    .poll(() =>
      api.requests.filter(
        (request) =>
          request.path === httpRoutes.synchronization &&
          request.method === "POST",
      ),
    )
    .toContainEqual({
      path: httpRoutes.synchronization,
      method: "POST",
      body: { scope: "workflows" },
    });
  await page
    .getByRole("button", { name: "#128 · Improve deployment reliability" })
    .click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await expect(
    page.getByText(/FAIL: deployment retry exhausted/),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Re-run workflow", exact: true }),
  ).toBeEnabled();
  await prepareScreenshot(page);
  await expect(page).toHaveScreenshot("workflow-logs.png", { fullPage: true });
  await page.getByRole("button", { name: "Close logs" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.getByRole("button", { name: "Clear", exact: true }).click();
  await page
    .getByRole("searchbox", { name: "Search", exact: true })
    .fill("missing");
  await expect(
    page.getByText("No workflows match the current filters."),
  ).toBeVisible();
});

test("navigation and issue discussion", async ({ page }) => {
  await openScreen(page, screens[0]);
  await page
    .getByRole("navigation")
    .getByRole("link", { name: "Issues" })
    .click();
  await page.getByRole("button", { name: /Retry failed deployments/ }).click();
  const dialog = page.getByRole("dialog");
  await expect(
    dialog.getByRole("heading", { name: "Deployment retries" }),
  ).toBeVisible();
  await expect(
    dialog.getByText("Investigating the timeout in the deploy job."),
  ).toBeVisible();
  await prepareScreenshot(page);
  await expect(page).toHaveScreenshot("issue-details.png", { fullPage: true });
  await page.getByRole("button", { name: "Close issue details" }).click();
  await expect(dialog).toHaveCount(0);
});

test("pull request details and checks", async ({ page }) => {
  await openScreen(page, screens[1]);
  await page.getByRole("button", { name: /Improve build checks/ }).click();
  await expect(
    page
      .getByRole("dialog")
      .getByRole("heading", { name: "Build checks", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Update branch", exact: true }),
  ).toBeEnabled();
  await expect(
    page.getByRole("button", { name: "Merge PR", exact: true }),
  ).toBeDisabled();
  await prepareScreenshot(page);
  await expect(page).toHaveScreenshot("pull-request-details.png", {
    fullPage: true,
  });
  await page
    .getByRole("button", { name: "Update branch", exact: true })
    .click();
  await expect(
    page.getByRole("dialog", { name: "Confirm Update branch", exact: true }),
  ).toBeVisible();
  await expect(page).toHaveScreenshot("update-branch-confirmation.png", {
    fullPage: true,
  });
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(
    page.getByRole("dialog", { name: "Confirm Update branch", exact: true }),
  ).toHaveCount(0);
});

test("workflow rerun requires confirmation and updates the run", async ({
  page,
  api,
}) => {
  await openScreen(page, screens[0]);
  await page
    .getByRole("button", { name: "#128 · Improve deployment reliability" })
    .click();
  await page
    .getByRole("button", { name: "Re-run workflow", exact: true })
    .click();
  const confirmation = page.getByRole("dialog", {
    name: "Confirm Re-run workflow",
    exact: true,
  });
  await expect(confirmation).toBeVisible();
  expect(
    api.requests.filter((request) => request.path === httpRoutes.executeAction),
  ).toEqual([]);
  await prepareScreenshot(page);
  await expect(page).toHaveScreenshot("rerun-confirmation.png", {
    fullPage: true,
  });
  await confirmation
    .getByRole("button", { name: "Cancel", exact: true })
    .click();
  await expect(confirmation).toHaveCount(0);
  expect(
    api.requests.filter((request) => request.path === httpRoutes.executeAction),
  ).toEqual([]);
  await page
    .getByRole("button", { name: "Re-run workflow", exact: true })
    .click();
  await confirmation
    .getByRole("button", { name: "Confirm action", exact: true })
    .click();
  await expect(
    page.getByRole("dialog").getByText("Queued", { exact: true }),
  ).toBeVisible();
  expect(
    api.requests.filter((request) => request.path === httpRoutes.executeAction),
  ).toEqual([
    {
      path: httpRoutes.executeAction,
      method: "POST",
      body: {
        sourceId: "github",
        repositoryId: "api",
        target: { type: "workflowRun", runId: "run-0" },
        action: "rerunWorkflow",
        revision: "abc123456789",
      },
    },
  ]);
  await page.getByRole("button", { name: "Close logs" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(
    page.locator(".workflow-row-summary").getByText("Queued", { exact: true }),
  ).toBeVisible();
});

test("settings save, feature navigation and reload", async ({ page }) => {
  await openScreen(page, screens[4]);
  await page.getByRole("checkbox", { name: /Issue monitoring/ }).uncheck();
  await page.getByRole("button", { name: "Save settings" }).click();
  await expect(page.getByText("Monitoring settings saved.")).toBeVisible();
  await expect(
    page.getByRole("navigation").getByRole("link", { name: "Issues" }),
  ).toHaveCount(0);
  await page.reload();
  await expect(
    page.getByRole("checkbox", { name: /Issue monitoring/ }),
  ).not.toBeChecked();
});

test("workflow service failure and recovery", async ({ page }) => {
  await page.route(`**${httpRoutes.workflows}`, (route) =>
    route.fulfill({
      status: 503,
      json: { message: "The source is temporarily unavailable." },
    }),
  );
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Unable to load workflow activity" }),
  ).toBeVisible();
  await prepareScreenshot(page);
  await expect(page).toHaveScreenshot("workflows-error.png", {
    fullPage: true,
  });
  await page.unroute(`**${httpRoutes.workflows}`);
  await page.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(page.getByText("Build and test", { exact: true })).toBeVisible();
});
