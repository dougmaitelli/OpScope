# Development

## Local setup

Prerequisites:

- Rust with rustfmt and Clippy (pinned in `rust-toolchain.toml`).
- Node.js and npm, matching the CI jobs.
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
`127.0.0.1:4317`. The server stores development data in `.opscope-data/`.

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
warnings denied, Rust tests, TypeScript compilation, ESLint, Prettier, and frontend tests.
The build command builds the Rust workspace and production frontend.

### Frontend tests

The shared web/desktop frontend uses Vitest, React Testing Library, and user-event
with a jsdom environment. Keep tests beside the code they cover as `*.test.ts`
(client logic) or `*.test.tsx` (React components). Common DOM matchers and cleanup
live in `apps/web/src/test/setup.ts`. Tests are type-checked and linted with the app.

```sh
npm test --workspace @opscope/web
npm run test:watch --workspace @opscope/web
```

Prefer accessible roles and labels, user interactions, and observable behavior
over component internals or snapshots. Mock network boundaries, not the components
under test. The same suite runs in CI through `npm run check`; it does not require
GitHub credentials or a running server. These are DOM interaction tests, not
real-browser, layout, or native desktop end-to-end tests.

Provider adapters also have Rust contract tests against scripted local HTTP
servers. They verify request authentication, routes, pagination, response mapping,
and bounded log reads without live provider credentials. Run them with
`cargo test -p opscope-core integrations`; the environment must permit binding
loopback sockets.

### Browser tests and README screenshots

```sh
npm run test:e2e:docker
npm run screenshots:docker
```

Playwright validates the production frontend in a desktop Chromium
viewport with behavior assertions and committed visual baselines. API fixtures
provide controlled data; the suite does not run the Rust server or native shell.
The screenshot task exports demonstration images used by the README separately
from regression baselines. See [browser testing](../e2e/README.md) for local
debugging, reviewed baseline updates, coverage, and failure reports.

| Command | Purpose |
| --- | --- |
| `npm run contracts:generate` | Regenerate committed TypeScript DTOs, routes, commands, and the client interface after changing Rust contracts. |
| `npm run contracts:check` | Detect generated-contract drift. |
| `npm run desktop:build` | Build a desktop executable without packaging an installer. |
| `npm run icons:generate` | Regenerate web and desktop icons from `assets/opscope-logo.png`. |

## Code organization

The workspace has one shared Rust library, `crates/opscope-core`, and two
executable crates, `apps/server` and `apps/desktop`. The React application in
`apps/web` is shared by both. See [architecture](architecture.md) for dependency
boundaries, persistence, and source-module extension points.

## Continuous delivery

[CI](../.github/workflows/ci.yml) runs on pushes and pull requests.
[Release](../.github/workflows/release.yml) runs on `v*` tags and checks that
the tag matches `Cargo.toml`, `package.json`, and
`apps/desktop/tauri.conf.json`. Keep corresponding lockfile versions in sync
when changing versions.

The release job fetches the full Git history and generates notes for the tagged
release with git-cliff using `cliff.toml`. It sets the draft release body from
that output, including when an existing draft is reused.

After validation, the release workflow builds:

- A universal macOS `.dmg` (Apple Silicon and Intel).
- A Windows x86-64 portable `.exe`.
- A Linux x86-64 AppImage and raw executable.
- A `linux/amd64` container at `ghcr.io/dougmaitelli/opscope`.
