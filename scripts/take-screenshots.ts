import { mkdir } from "node:fs/promises";
import { test, prepareScreenshot } from "../e2e/fixtures";
import { screens, openScreen } from "../e2e/screens";

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
    await prepareScreenshot(page);
    await page.screenshot({
      path: `screenshots/${screen.name}.png`,
      fullPage: true,
      animations: "disabled",
      caret: "hide",
    });
  }
});
