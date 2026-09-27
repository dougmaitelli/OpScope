# Development

## Prerequisites

- Rust 1.98.1 with `rustfmt` and `clippy` (also declared in
  `rust-toolchain.toml`).
- Node.js 24 and npm.
- The platform prerequisites listed by Tauri for desktop builds.

Install JavaScript dependencies and generate the committed TypeScript contract:

```sh
npm install
npm run contracts:generate
```

No global Tauri installation is required; the CLI is pinned in the root npm
workspace.

## Workspace checks

Run the complete local and CI review gate:

```sh
npm run check
```

This verifies the generated contract, Rust formatting, Clippy with warnings
denied, all Rust tests, and strict TypeScript compilation. Build all Rust crates
and the production frontend with:

```sh
npm run build
```

The other root commands are:

| Command | Purpose |
| --- | --- |
| `npm run contracts:generate` | Regenerate TypeScript DTOs, routes, commands, and the client interface from Rust. |
| `npm run contracts:check` | Fail if the committed TypeScript contract differs from Rust. |
| `npm run desktop:build` | Build the desktop application executable without packaging an installer. |
| `npm run server:dev` | Run the loopback-only development HTTP server on port 4317. |
| `npm run web:dev` | Run the browser frontend on port 1420 and proxy `/api` to the development server. |
| `npm run desktop:dev` | Run the Tauri desktop shell and shared Vite frontend. |

The HTTP server binds to `127.0.0.1` for direct development and is intended to
sit behind the self-hosted reverse proxy later. OIDC is disabled when issuer,
client ID, and client secret are all unset. When any of those three is set, all
four required values must be present and valid or startup fails:

| Variable | Purpose |
| --- | --- |
| `CIWATCHER_PUBLIC_URL` | Required with OIDC. Browser-visible origin. HTTPS is required except for loopback development. The callback is `<origin>/api/auth/callback`. |
| `CIWATCHER_OIDC_ISSUER` | Required with OIDC. Exact issuer URL used for discovery and ID-token validation. |
| `CIWATCHER_OIDC_CLIENT_ID` | Required with OIDC. Confidential web-client identifier. |
| `CIWATCHER_OIDC_CLIENT_SECRET` | Required with OIDC. Confidential web-client secret. |
| `CIWATCHER_OIDC_ALLOWED_SUBJECTS` | Optional comma-separated `sub` allowlist. When unset, provider-side client access policy controls admission. |

For local development, register
`http://127.0.0.1:1420/api/auth/callback` with the provider and set
`CIWATCHER_PUBLIC_URL=http://127.0.0.1:1420`. Vite proxies the callback and API
requests to the Rust server.

With OIDC disabled, the web API and interface do not require a session or CSRF
token. Use that mode only where network access is already restricted to trusted
users.

The development server stores metadata and encrypted credentials under
`.ciwatcher-data/`. It creates a 32-byte `master.key` with owner-only
permissions on first use. Set `CIWATCHER_DATA_DIR` to move the database or
`CIWATCHER_MASTER_KEY_FILE` to use a separately mounted raw 32-byte key file.
Production self-hosting will require the latter rather than the development
default.

Set `CIWATCHER_APPRISE_URL` to an Apprise API notification endpoint, such as
`http://127.0.0.1:8000/notify/ciwatcher`, to enable server notifications. Leave
it unset to disable delivery. Apprise service URLs and credentials stay in the
Apprise configuration identified by that endpoint rather than in CI Watcher.
When using the stateless `/notify` endpoint, configure those destinations on
the Apprise API process with its `APPRISE_STATELESS_URLS` environment variable.

## Dependency direction

The workspace has one shared library and two executable crates:

```text
Cargo.toml
├── crates/ciwatcher-core/Cargo.toml
├── apps/server/Cargo.toml
└── apps/desktop/Cargo.toml
```

Inside `ciwatcher-core`, module dependencies point inward:

```text
domain <── application <── integrations
   ^             ^
   └── contracts └── persistence
```

- The domain module has no dependencies on other project modules.
- The application module defines ports and depends only on domain concepts.
- Integrations and persistence implement application ports and may depend on
  domain types.
- Response contracts contain no secret material; the connection request is the
  sole credential-bearing transport type.
- Axum and Tauri occur only in their executable crates.

Source modules are added to the central compiled catalog in
`integrations/mod.rs`. The shared registry exposes them to both editions
through generic list, connect, and disconnect contracts, so the frontend
renders new registered sources without provider-specific UI code.
Modules also declare their connection fields and own their validation and
normalization. The GitHub module defaults its server URL to `https://github.com`;
an enterprise origin such as `https://github.example.com` uses the REST prefix
`/api/v3`.

These are module-level rules rather than separate compilation units. If an
integration later requires enough isolated dependencies or conditional builds
to justify a crate, it can be extracted then.

The frontend implements the generated `ApplicationClient` twice: once with
same-origin HTTP and once with Tauri IPC. The desktop application does not run
an HTTP server.
