import { mkdir } from "node:fs/promises";
import type { Page } from "@playwright/test";
import { test, prepareScreenshot } from "../e2e/fixtures";
import { screens, openScreen } from "../e2e/screens";

async function addWindowFrame(page: Page) {
  const viewport = page.viewportSize()!;
  await page.evaluate(
    ({ width, height }) => {
      const root = document.getElementById("root");
      if (!root) throw new Error("Application root is missing");

      document.documentElement.classList.add("documentation-screenshot");
      document.body.classList.add("documentation-screenshot");
      document.documentElement.style.setProperty(
        "--screenshot-content-width",
        `${width}px`,
      );
      document.documentElement.style.setProperty(
        "--screenshot-content-height",
        `${height}px`,
      );

      const frame = document.createElement("div");
      frame.className = "screenshot-window";
      const titlebar = document.createElement("div");
      titlebar.className = "screenshot-titlebar";
      titlebar.innerHTML = `
      <div class="screenshot-controls" aria-hidden="true"><span></span><span></span><span></span></div>
      <span class="screenshot-title">OpScope</span>
    `;
      root.before(frame);
      frame.append(titlebar, root);
    },
    viewport,
  );
  await page.addStyleTag({ path: "scripts/screenshot-frame.css" });

  const size = await page.locator(".screenshot-window").evaluate((element) => {
    const bounds = element.getBoundingClientRect();
    const body = getComputedStyle(document.body);
    return {
      width: Math.ceil(
        bounds.width + parseFloat(body.paddingLeft) + parseFloat(body.paddingRight),
      ),
      height: Math.ceil(
        bounds.height + parseFloat(body.paddingTop) + parseFloat(body.paddingBottom),
      ),
    };
  });
  await page.setViewportSize(size);
}

test("export README screenshots", async ({ page }) => {
  await mkdir("screenshots", { recursive: true });
  for (const screen of screens.filter((screen) => screen.name !== "settings")) {
    await page.setViewportSize({ width: 1400, height: 900 });
    await openScreen(page, screen);
    // The application scrolls its workspace rather than the document. Expand
    // the export viewport so the complete monitoring content is included.
    const height = await page
      .locator(".workspace")
      .evaluate((element) => element.scrollHeight);
    await page.setViewportSize({ width: 1400, height: Math.max(900, height) });
    await addWindowFrame(page);
    await prepareScreenshot(page);
    await page.screenshot({
      path: `screenshots/${screen.name}.png`,
      fullPage: true,
      omitBackground: true,
      animations: "disabled",
      caret: "hide",
    });
  }
});
