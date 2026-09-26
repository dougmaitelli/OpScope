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
| `npm run desktop:dev` | Run the Tauri desktop shell and shared Vite frontend. |

The current HTTP server has no authentication and therefore binds exclusively
to `127.0.0.1`. It is not a deployable self-hosted edition.

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
domain <── application <── integrations/fake
   ^
   └──────── contracts
```

- The domain module has no dependencies on other project modules.
- The application module defines ports and depends only on domain concepts.
- Integrations implement application ports and may depend on domain types.
- Contracts map domain values into secret-free transport DTOs.
- Axum and Tauri occur only in their executable crates.

These are module-level rules rather than separate compilation units. If an
integration later requires enough isolated dependencies or conditional builds
to justify a crate, it can be extracted then.

The frontend implements the generated `ApplicationClient` twice: once with
same-origin HTTP and once with Tauri IPC. The desktop application does not run
an HTTP server.
