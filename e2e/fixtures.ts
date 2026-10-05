import { test as base, expect, type Page } from "@playwright/test";
import {
  httpRoutes,
  applicationVersion,
  type ActionOptionsRequest,
  type ActionOptions,
  type ExecuteActionRequest,
} from "../apps/web/src/generated/contracts";
import {
  inventory,
  issues,
  pullRequests,
  issueDetails,
  pullRequestDetails,
  settings,
  timestamp,
} from "./data";

export const test = base.extend<{
  api: { requests: { path: string; method: string; body: unknown }[] };
}>({
  api: [
    async ({ page }, use) => {
      const failures: string[] = [];
      const requests: { path: string; method: string; body: unknown }[] = [];
      let currentSettings = structuredClone(settings);
      const currentInventory = structuredClone(inventory);
      page.on("pageerror", (error) => failures.push(error.message));
      await page.clock.setFixedTime(new Date("2026-09-28T10:05:00Z"));
      await page.route("**/*", async (route) => {
        const request = route.request();
        const url = new URL(request.url());
        if (url.origin !== "http://127.0.0.1:1430") {
          failures.push(`Unexpected external request: ${url}`);
          await route.abort();
          return;
        }
        if (!url.pathname.startsWith("/api/")) {
          await route.continue();
          return;
        }
        const method = request.method();
        const body: unknown = request.postData()
          ? request.postDataJSON()
          : null;
        requests.push({ path: url.pathname, method, body });
        let response: unknown;
        switch (url.pathname) {
          case "/api/auth/session":
            response = {
              enabled: false,
              authenticated: false,
              user: null,
              csrfToken: null,
            };
            break;
          case httpRoutes.updateStatus:
            response = {
              currentVersion: applicationVersion,
              latestVersion: applicationVersion,
              updateAvailable: false,
              releaseUrl: "https://github.com/dougmaitelli/OpScope/releases",
            };
            break;
          case httpRoutes.settings:
            if (method === "PUT") currentSettings = request.postDataJSON();
            response = currentSettings;
            break;
          case httpRoutes.workflows:
            response = currentInventory;
            break;
          case httpRoutes.issues:
            response = { selectedRepositoryCount: 3, issues };
            break;
          case httpRoutes.changeRequests:
            response = {
              selectedRepositoryCount: 3,
              changeRequests: pullRequests,
            };
            break;
          case httpRoutes.activity:
            response = {
              changeRequestEvents: [
                {
                  id: "event-1",
                  kind: "changesRequested",
                  occurredAt: timestamp,
                  changeRequest: pullRequests[0],
                },
              ],
              issueEvents: [
                {
                  id: "event-2",
                  kind: "opened",
                  occurredAt: timestamp,
                  issue: issues[0],
                },
              ],
            };
            break;
          case httpRoutes.synchronization:
            response =
              method === "POST"
                ? {
                    selectedRepositoryCount: 3,
                    synchronizedRepositoryCount: 3,
                    failedRepositoryCount: 0,
                    skippedRepositoryCount: 0,
                    alreadyRunning: false,
                  }
                : {
                    running: false,
                    activeSourceCount: 0,
                    lastCompletedAt: Date.parse(timestamp) / 1000,
                    lastFailedRepositoryCount: 0,
                  };
            break;
          case httpRoutes.workflowRunLogs:
            const runId = (body as { runId: string }).runId;
            const run = currentInventory.workflows
              .flatMap((workflow) => workflow.runs)
              .find((run) => run.id === runId);
            response = {
              run,
              files:
                run?.lifecycle === "queued"
                  ? []
                  : [
                      {
                        name: "build.log",
                        content:
                          run?.outcome === "failure"
                            ? "Install dependencies\nRun integration checks\nFAIL: deployment retry exhausted\nProcess exited with code 1"
                            : "Install dependencies\nRun integration checks\nAll checks passed\nProcess exited with code 0",
                      },
                    ],
              truncated: false,
            };
            break;
          case httpRoutes.issueDetails:
            response = issueDetails;
            break;
          case httpRoutes.changeRequestDetails:
            response = pullRequestDetails;
            break;
          case httpRoutes.actionOptions:
            const { target } = body as ActionOptionsRequest;
            response = {
              revision: "abc123456789",
              actions:
                target.type === "workflowRun"
                  ? [
                      {
                        action: "rerunWorkflow",
                        label: "Re-run workflow",
                        confirmation: "Re-run all jobs at the original commit?",
                        disabledReason: null,
                      },
                    ]
                  : [
                      {
                        action: "updateBranch",
                        label: "Update branch",
                        confirmation:
                          "Merge main into the pull request branch? This does not merge the pull request.",
                        disabledReason: null,
                      },
                      {
                        action: "mergeChangeRequest",
                        label: "Merge PR",
                        confirmation:
                          "Merge this pull request into main using squash?",
                        disabledReason:
                          target.number === 17
                            ? "Required checks are failing and changes have been requested."
                            : null,
                      },
                    ],
            } satisfies ActionOptions;
            break;
          case httpRoutes.executeAction:
            const action = body as ExecuteActionRequest;
            if (
              action.action !== "rerunWorkflow" ||
              action.target.type !== "workflowRun"
            ) {
              failures.push(`Unexpected action: ${action.action}`);
              await route.fulfill({
                status: 400,
                json: { message: "Unsupported fixture action" },
              });
              return;
            }
            const rerun = currentInventory.workflows
              .flatMap((workflow) => workflow.runs)
              .find(
                (run) => run.id === (action.target as { runId: string }).runId,
              )!;
            rerun.attempt += 1;
            rerun.lifecycle = "queued";
            rerun.outcome = "unknown";
            response = {
              accepted: true,
              run: rerun,
              changeRequest: null,
              details: null,
            };
            break;
          case httpRoutes.sources:
            response = { sources: [] };
            break;
          case httpRoutes.repositories:
            response = { sources: [], warnings: [] };
            break;
          default:
            failures.push(`Unexpected API request: ${method} ${url.pathname}`);
            await route.fulfill({
              status: 500,
              json: { message: "Unexpected test request" },
            });
            return;
        }
        await route.fulfill({ json: response });
      });
      await use({ requests });
      expect(failures, "Browser exceptions and unexpected requests").toEqual(
        [],
      );
    },
    { auto: true },
  ],
});
export { expect };

export async function prepareScreenshot(page: Page) {
  await page.evaluate(async () => {
    await document.fonts.ready;
    await Promise.all(
      Array.from(document.images).map(async (image) => {
        if (!image.complete)
          await new Promise<void>((resolve, reject) => {
            image.addEventListener("load", () => resolve(), { once: true });
            image.addEventListener(
              "error",
              () => reject(new Error(`Image failed: ${image.src}`)),
              { once: true },
            );
          });
        if (!image.naturalWidth) throw new Error(`Image failed: ${image.src}`);
      }),
    );
  });
}

// Freeze release metadata to the original baseline value, while retaining its
// typography and layout. Documentation exports use prepareScreenshot instead.
export async function prepareVisualScreenshot(page: Page) {
  await prepareScreenshot(page);
  await page.locator(".sidebar-version > span").evaluate((element) => {
    element.textContent = "Version 0.7.0";
  });
}
