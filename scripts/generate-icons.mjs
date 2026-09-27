import {
  copyFileSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
} from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const source = join(repositoryRoot, "assets", "opsscope-logo.png");
const desktopIcons = join(repositoryRoot, "apps", "desktop", "icons");
const webIcon = join(repositoryRoot, "apps", "web", "public", "opsscope-logo.png");
const generatedIcons = mkdtempSync(join(tmpdir(), "opsscope-icons-"));
const tauri = join(
  repositoryRoot,
  "node_modules",
  ".bin",
  process.platform === "win32" ? "tauri.cmd" : "tauri",
);

if (!existsSync(tauri)) {
  throw new Error("Tauri CLI is unavailable; run npm install first");
}

const generation = spawnSync(tauri, ["icon", source, "--output", generatedIcons], {
  cwd: repositoryRoot,
  stdio: "inherit",
});

if (generation.error) throw generation.error;
if (generation.status !== 0) process.exit(generation.status ?? 1);

const filesMatch = (left, right) =>
  existsSync(right) && readFileSync(left).equals(readFileSync(right));
const desktopArtworkChanged = !filesMatch(
  join(generatedIcons, "icon.png"),
  join(desktopIcons, "icon.png"),
);

if (desktopArtworkChanged) {
  for (const entry of readdirSync(generatedIcons, { withFileTypes: true })) {
    if (entry.isFile()) {
      copyFileSync(join(generatedIcons, entry.name), join(desktopIcons, entry.name));
    }
  }
}

const generatedWebIcon = join(generatedIcons, "128x128@2x.png");
if (!filesMatch(generatedWebIcon, webIcon)) copyFileSync(generatedWebIcon, webIcon);

rmSync(generatedIcons, { recursive: true, force: true });
console.log("Generated desktop icons and web favicon from assets/opsscope-logo.png");
