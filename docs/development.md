# Development

Deployment, token permissions, authentication, and runtime configuration belong
in the [README](../README.md#deployment). This guide covers working on the code.

## Local setup

Prerequisites:

- Rust 1.98.1 with rustfmt and Clippy (pinned in `rust-toolchain.toml`).
- Node.js 24 and npm, matching the CI jobs.
- Platform build dependencies for Tauri when building the desktop crate.
  On the Ubuntu CI runners these include `libwebkit2gtk-4.1-dev`,
  `libappindicator3-dev`, `librsvg2-dev`, `patchelf`, and `xdg-utils`.

Install the locked dependencies:

```sh
npm ci
```

For the browser UI, run these in separate terminals:

```sh
npm run server:dev
npm run web:dev
```

Open `http://127.0.0.1:1420`. Vite proxies `/api` to the Rust server on
`127.0.0.1:4317`. The server stores development data in `.opsscope-data/`.
No OIDC configuration is needed for local development.

To test OIDC locally, use the configuration described in the README with
`OPSSCOPE_PUBLIC_URL=http://127.0.0.1:1420` and register
`http://127.0.0.1:1420/api/auth/callback` with your provider.

For the desktop UI:

```sh
npm run desktop:dev
```

This starts the shared frontend and Tauri shell; it does not need the HTTP
server. The Tauri CLI is installed locally by npm.

## Checks and builds

```sh
npm run check
npm run build
```

The check command verifies generated contracts, Rust formatting, Clippy with
warnings denied, Rust tests, TypeScript compilation, ESLint, and Prettier.
The build command builds the Rust workspace and production frontend.

| Command | Purpose |
| --- | --- |
| `npm run contracts:generate` | Regenerate committed TypeScript DTOs, routes, commands, and the client interface after changing Rust contracts. |
| `npm run contracts:check` | Detect generated-contract drift. |
| `npm run desktop:build` | Build a desktop executable without packaging an installer. |
| `npm run icons:generate` | Regenerate web and desktop icons from `assets/opsscope-logo.png`. |

## Code organization

The workspace has one shared Rust library, `crates/opsscope-core`, and two
executable crates, `apps/server` and `apps/desktop`. The React application in
`apps/web` is shared by both. See [architecture](architecture.md) for dependency
boundaries, persistence, and source-module extension points.

Keep feature-specific UI components and styles with their feature, and reuse
shared components for common row, pill, and project-group presentation.
Changes to application behavior belong in the shared core; platform adapters
provide transport, credential storage, and notification delivery.

## Continuous delivery

[CI](../.github/workflows/ci.yml) runs on pushes and pull requests.
[Release](../.github/workflows/release.yml) runs on `v*` tags and checks that
the tag matches `Cargo.toml`, `package.json`, and
`apps/desktop/tauri.conf.json`. Keep corresponding lockfile versions in sync
when changing versions.

After validation, the release workflow builds:

- A universal macOS `.app.zip` (Apple Silicon and Intel).
- A Windows x86-64 portable `.exe`.
- A Linux x86-64 AppImage and raw executable.
- A `linux/amd64` container at `ghcr.io/dougmaitelli/opsscope`.

Container version tags use a `v` prefix: full version (such as `v0.3.1`),
major/minor (`v0.3`), and major (`v0`). Stable releases also publish `latest`.
The GitHub release remains a draft until all desktop builds
and container publishing succeed. macOS builds use ad-hoc signing; trusted
publisher signing and Apple notarization are not configured.
