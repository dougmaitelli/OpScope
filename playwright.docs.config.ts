import { defineConfig } from "@playwright/test";
import config from "./playwright.config";

export default defineConfig({
  ...config,
  testDir: "./scripts",
  testMatch: "take-screenshots.ts",
  projects: [config.projects![0]!],
  outputDir: "test-results/docs",
  reporter: [["list"]],
});
