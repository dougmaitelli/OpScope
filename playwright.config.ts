import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  testMatch: "**/*.spec.ts",
  fullyParallel: true,
  workers: process.env.CI ? 2 : undefined,
  forbidOnly: !!process.env.CI,
  retries: 0,
  updateSnapshots: "none",
  snapshotPathTemplate: "{testDir}/snapshots/{projectName}/{arg}{ext}",
  reporter: [["list"], ["html", { open: "never" }]],
  expect: {
    toHaveScreenshot: {
      animations: "disabled",
      caret: "hide",
      maxDiffPixels: 0,
    },
  },
  use: {
    browserName: "chromium",
    colorScheme: "dark",
    baseURL: "http://127.0.0.1:1430",
    locale: "en-US",
    timezoneId: "UTC",
    reducedMotion: "reduce",
    serviceWorkers: "block",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  projects: [
    { name: "desktop", use: { viewport: { width: 1400, height: 900 } } },
  ],
  webServer: {
    command:
      "npm run build --workspace @opscope/web && npm exec --workspace @opscope/web -- vite preview --host 127.0.0.1 --port 1430 --strictPort",
    url: "http://127.0.0.1:1430",
    reuseExistingServer: false,
    timeout: 120_000,
  },
});
