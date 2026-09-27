# Architecture

## Context

CI Watcher has one Rust application core and two delivery shells:

- A native-WebView desktop application using narrow IPC commands and an
  operating-system secret store.
- A self-hosted server exposing a same-origin HTTP API and serving the shared
  web application.

The first integration is GitHub and the first monitor type is GitHub Actions.

## Dependency rule

Dependencies point inward:

```text
GitHub API -> integration adapter -> application services <- persistence port
                                      ^        ^
                                      |        |
                              Desktop adapter  HTTP adapter
                                      \        /
                                      shared UI
```

The domain and application modules cannot depend on a desktop framework, HTTP
framework, database driver, UI framework, or GitHub response types.

## Repository structure

```text
crates/
  ciwatcher-core/
    domain                Pure types, invariants, and state transitions
    application           Use cases and port interfaces
    contracts             Stable request and response DTOs
    integrations          Provider adapters, beginning with GitHub
    persistence           SQLite metadata and encrypted server-secret adapters
apps/
  server/                 Server composition root and HTTP adapter
  desktop/                Desktop composition root and IPC adapter
  web/                    Shared web application
```

Domain, application, contracts, and integrations are modules in one shared Rust
crate. They are conceptual boundaries, not separate Cargo packages. A module is
extracted into another crate only when concrete dependency, build, or reuse
requirements justify the additional package boundary. The dependency rule is
more important than the number of packages.

## Application ports

Initial ports are expected to cover:

- Clock and identifier generation.
- Connection validation and resource discovery.
- Monitor and activity synchronization.
- Workspace, connection, resource, monitor, observation, activity, and sync
  repositories.
- Secret storage by opaque secret reference.
- Unit-of-work or transaction boundaries.
- Application event publication.

Source integrations implement the `SourceModule` interface. A module supplies
its stable identifier, display metadata, credential-field description,
credential validation behavior, repository discovery, and workflow discovery.
A central compiled catalog registers modules in `SourceRegistry` for both
editions; generic list, connect, disconnect, and discovery use cases drive both
transports and the UI. Adding a source must not require a provider-specific
route, IPC command, DTO, or frontend screen.

Provider credentials are represented by opaque references outside the secret
adapter. DTOs returned to a transport contain credential status and metadata,
never secret material. Credential input exists only in the intentional
connection request.

## Frontend transports

The web application depends on a typed client interface rather than directly on
HTTP or a desktop framework:

- The web implementation uses same-origin JSON HTTP requests.
- The desktop implementation invokes an explicit allowlist of IPC commands.

Both adapters call the same application use cases and map the same DTOs. The
desktop application does not start a loopback HTTP server.

## Credential separation

Administrator password verification, browser sessions, provider credentials,
server-side credential encryption, and future webhook verification use
independent secrets and lifecycles. The desktop stores provider credentials in
the operating-system keychain. The self-hosted server uses a separately managed
encryption key; the administrator password is never an encryption key. See the
[security model](security.md) for the complete controls.

## Persistence model

Likely version-one tables are:

- `workspaces`
- `connections`
- `secret_references`
- `resources`
- `monitors`
- `observations`
- `activities`
- `sync_states`
- `sessions` (server only)
- `audit_events`
- schema migration metadata

Records use internal UUIDs and store provider-issued durable numeric IDs where
available. Mutable names, slugs, and URLs are not identities.

Provider-specific details use versioned payloads only where a common column is
not meaningful. Fields that are filtered, sorted, constrained, or joined belong
in typed columns rather than opaque JSON.

SQLite is the only version-one database. Database access remains behind
application ports, but no lowest-common-denominator SQL abstraction for a
hypothetical future PostgreSQL implementation will be created prematurely.

## Synchronization

Synchronization is a background application concern, not a UI concern.

- Only one sync for a connection may run at a time.
- Manual refresh uses the same synchronization path as scheduled refresh.
- Latest-run failure transitions are persisted and grouped into one
  provider-independent notification per repository.
- Desktop and server shells implement notification delivery through native
  system notifications and an environment-configured Apprise endpoint,
  respectively.
- Cached data remains available during provider outages.
- Last attempt and last successful sync are distinct.
- Conditional requests and ETags are used where supported.
- GitHub rate-limit response data is recorded.
- Transient failures use bounded exponential backoff with jitter.
- Authentication and authorization failures do not retry indefinitely.
- Shutdown cancels work cleanly and does not leave partial projections.
- Provider activity is upserted idempotently by durable external ID.

The initial default interval is 60 seconds and will be configurable within safe
bounds. Self-hosted webhooks are a later optimization; polling remains required
for desktop operation.

## Extension model

Integrations are compiled into the application and registered explicitly. There
is no version-one runtime plugin ABI or execution of downloaded third-party
code.

Source and monitor type are separate dimensions. Adding GitHub deployments, for
example, extends the GitHub integration with a new monitor capability rather
than pretending that GitHub Deployments is a different provider.

## Technology choice

The selected implementation stack is:

- Rust for the domain, application core, integrations, and native adapters.
- Tauri 2 for the native desktop shell using an operating-system WebView.
- Axum, Tokio, and Tower for the self-hosted HTTP composition.
- A Rust application core reused by desktop and server compositions.
- React with TypeScript and Vite for the shared component-based frontend.
- SQLite as the version-one database.
- TypeScript DTOs, route names, command names, and the frontend client interface
  generated from Rust with `ts-rs`, with a CI drift check covering desktop IPC
  and HTTP transports.

Rust was selected after building equivalent disposable Rust and Go prototypes.
Go built considerably faster, but build time was not an important project
constraint. On the comparison machine, the minimal Rust server used 3.00 MiB
idle RSS and produced a 1.69 MiB executable, versus 11.19 MiB and 6.17 MiB for
Go. These measurements are directional rather than production-load guarantees.
Rust also provides the preferred Tauri capability model and compile-time domain
modeling.

## Testing strategy

- Unit tests for domain invariants and normalization.
- Application tests with in-memory ports.
- GitHub contract tests against synthetic fixtures and a mock HTTP server.
- SQLite migration, transaction, and idempotency tests.
- Security tests for session, CSRF, redaction, and authorization behavior.
- The same frontend client contract suite against HTTP and desktop transports.
- End-to-end smoke tests for desktop and self-hosted packaging.
- Backup restoration tests before a release changes the database schema.
