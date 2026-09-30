# Architecture

OpsScope has one Rust application core and two delivery shells. Desktop and
self-hosted editions share a React/TypeScript frontend, generated contracts,
and application behavior; neither edition depends on the other.

## Structure and boundaries

```text
crates/opsscope-core/src/
  domain/          Provider-independent types
  application/     Use cases, ports, caching, synchronization, notifications
  contracts/       Transport DTOs and generated client definitions
  integrations/    Compiled source modules and provider adapters
  persistence.rs   SQLite setup and persistence adapters
  persistence/     Persistence implementation modules and tests
apps/
  server/          Axum HTTP adapter, OIDC, Apprise, server composition
  desktop/         Tauri IPC adapter, OS keychain, tray, native notifications
  web/             Shared React UI, feature pages, components, client adapters
```

Domain and application code must not depend on Axum, Tauri, UI code, or
provider response types. Application ports separate business behavior from
storage and external services. Integrations and persistence implement those
ports. These are module boundaries within one shared crate, not separate
crates for each layer.

The frontend uses a generated `ApplicationClient` interface. Its web adapter
uses same-origin HTTP; its desktop adapter uses allowlisted Tauri IPC.
Desktop does not start an HTTP server. Rust contracts are exported with
`ts-rs`; CI detects drift in DTOs, routes, commands, and the client interface.

## Sources and connections

A `SourceModule` describes a provider, its connection fields, validation,
normalization, and monitoring capabilities. The compiled catalog in
`integrations/mod.rs` registers modules for both editions. Generic connection
contracts and module-provided fields keep provider-specific forms out of the UI.

A connection is one configured instance of a source. Connections have internal
IDs; uniqueness is enforced by module ID plus a module-owned connection key.
For GitHub, that key is the normalized server origin. GitHub.com and separate
Enterprise Server origins can coexist.

GitHub currently supplies workflows, pull requests, and issues. New providers
implement the relevant application ports; new capabilities may require new
shared domain and UI behavior. There is no runtime plugin loading.

## Storage and cache

SQLite stores connections, repository selections, settings, cached source
data, activity events, notification observations, and credential audit events.
Provider IDs identify cached objects; names and URLs are presentation data.

Application use cases read source data through a port. A caching decorator
handles SQLite snapshots, cache lifetimes, and API refreshes transparently;
callers do not choose between the database and GitHub. This allows the cache
implementation to be changed independently.

The UI can render cached snapshots before background synchronization completes.
Repository metadata has a longer cache lifetime than workflow and run data.
Manual refresh bypasses cache TTLs. Failed refreshes preserve the last
successful snapshot, with freshness and failure information available to the UI.
Run logs are fetched on demand and are not persisted.

Desktop credentials use the OS keychain. Server credentials use encrypted
SQLite storage with a master key independent of OIDC. Transport responses
contain connection metadata and credential status, never stored tokens.
OIDC sessions are held in server memory, not SQLite.

## Synchronization and notifications

Both shells run the shared synchronization coordinator on the interval stored
in Settings. Manual refresh uses the same coordinator. It prevents overlapping
synchronization for a connection and continues when an individual repository
fails. The UI observes synchronization status rather than owning the schedule.

The core detects new failure observations for actual latest workflow runs,
persists notification state, and groups failures by repository. Platform
implementations of the notification port deliver native notifications or
Apprise requests. Initial observations establish a baseline rather than
alerting on existing failures.

PR and issue notifications have independent, account-scoped SQLite observation
state, separate from Activity. Successful open-item lists are compared with prior
observations; missing items require a detail lookup before detecting closure or
merge. Unresolved items remain eligible for a later lookup. Closed issue state is
retained to detect subsequent reopens. Failed list fetches do not advance that
resource kind's baseline.

Settings select individual event types. Observations advance even for disabled
or personally irrelevant events, and are persisted before delivery for
at-most-once notification attempts. PR and issue events are grouped per repository
and delivered as informational messages through the same platform notification
port. Workflow failure grouping remains separate.

The Activity view combines workflow runs with locally observed PR changes;
it is not a complete provider audit log.

## Personal scope

“Me” is the token owner for each connection, not a desktop OS identity or OIDC
login. Provider adapters collect relationship evidence: authors, reviewers,
review assignments and decisions, discussion participants, commit authors, and
linked PRs/MRs. These facts are cached with the resource, not reduced to a
persisted relevance decision. The core evaluates them locally against an account
ID for views, counts, history, activity, and notification eligibility; evaluation
does not make provider calls.

Viewer-only observations (such as subscriptions and GitHub viewer flags) retain
the observing account ID. Each actor set records whether it is complete, so an
absent actor in a partial response is not a confirmed non-match. GitLab commit
email matching uses cached provider-confirmed account email aliases.

Evidence gathering does not stop when the current viewer matches. This retains
facts usable for other accounts and future local queries. Caches remain scoped
to the connection and account: reusable data is not authorization to share a
private repository with another user. An upgrade discards legacy relevance
decisions and expires the affected snapshots without deleting resource data,
connections, or notification history.

Filtering does not discard unrelated cached source data. Unknown relevance is
excluded while the setting is enabled. A workflow view can show the latest
matching historical run, but notifications still consider only the actual
latest run. This prevents old matching failures from becoming new alerts.

## Testing

The workspace check runs Rust tests across core and platform adapters, including
application behavior, synthetic provider responses, persistence, and server
authentication. TypeScript checks, linting, formatting, and generated-contract
checks cover the shared frontend contract. Build checks cover both shells.

Live-provider compatibility, desktop OS integration, and deployment behavior
still need appropriate smoke testing; passing unit tests is not proof of those
environments. See [development](development.md) for commands and
[security](security.md) for trust boundaries.
