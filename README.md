# OpsScope

<p align="center">
  <img src="assets/opsscope-logo.png" alt="OpsScope logo" width="128" height="128">
</p>

OpsScope is a local-first application for monitoring software-delivery
systems. It begins with GitHub Actions across multiple repositories and runs as
either a desktop application or a self-hosted web application.

The first release should make it easy to see what is failing, what is running,
what changed recently, which repository needs attention, and how fresh the
displayed information is.

## Principles

- Read-only before requesting permission to change external systems.
- Desktop operation does not depend on a service run by this project.
- Desktop and self-hosted editions share one application core and frontend.
- Cached data always exposes its freshness and remains useful during outages.
- Stored credentials are never returned to the frontend.
- Missing production security configuration fails closed.

## Version-one scope

Version one includes:

- Multiple GitHub.com and GitHub Enterprise Server connections using personal
  access tokens.
- Repository and GitHub Actions workflow discovery.
- Current and recent workflow-run state across selected repositories.
- On-demand workflow-run logs with links back to the source provider.
- Repository, workflow, branch, commit, actor, trigger, timing, outcome,
  freshness, and links back to GitHub.
- Manual and scheduled refresh with cached state during transient failures.
- New latest-run failure notifications, grouped once per repository.
- Search and filtering by repository, workflow, and status.
- Configurable local history stored in SQLite.
- Optional **Only show my work** scope in Settings, using each connection's token
  owner for PR authorship/reviews/review requests, issue authorship/subscriptions/
  discussion, and workflow commit/PR ownership. It also scopes native and Apprise
  notifications without discarding other monitored data.
- Native desktop and single-administrator self-hosted editions.

It does not include write actions, artifacts, multiple users,
PostgreSQL, other providers, runtime plugins, mobile applications, or a hosted
service.

Personal scope shows the latest matching workflow run and matching cached history,
not necessarily the repository's latest run. Notifications still consider only the
actual latest run per workflow. Items with unavailable relevance metadata are hidden
in personal scope until a successful refresh; switching back restores the full view.

## Model

The core uses general monitoring language rather than CI-specific language:

| Term | Meaning | Initial GitHub mapping |
| --- | --- | --- |
| Workspace | Ownership and authorization boundary | The single local workspace |
| Source | External system providing information | GitHub |
| Connection | Source configuration and secret reference | GitHub account and token |
| Resource | Durable provider object that can be monitored | Repository |
| Monitor | Configured observation of a resource | Actions workflow |
| Observation | Latest known state and freshness | Latest workflow state |
| Activity | Historical immutable occurrence | Workflow run |
| Sync state | Cursor, rate limit, retry, and freshness metadata | GitHub synchronization state |

Lifecycle (`queued`, `running`, `completed`, or `unknown`) is separate from
outcome (`success`, `warning`, `failure`, `cancelled`, `skipped`, or `unknown`).
Provider-native values are retained alongside normalized values.

## Documentation

- [Architecture](docs/architecture.md)
- [Security and threat model](docs/security.md)
- [Development guide](docs/development.md)

## Development

The workspace currently provides the shared Rust core, Axum server, Tauri
desktop shell, generated TypeScript contracts, and the GitHub source module.
GitHub account validation, multi-server connection persistence, repository selection,
workflow discovery, and recent workflow-run monitoring are implemented. Run
activity and synchronization freshness are cached in SQLite, with the last
successful snapshot retained during transient provider failures. Users can
inspect workflow-run logs on demand without persisting them locally. Desktop
credentials use the operating-system keychain, minimizes to the system tray,
and delivers native failure notifications. The server encrypts credentials
stored in SQLite, optionally authenticates browser sessions through OpenID Connect, and
can deliver failure notifications through Apprise. Manual refresh uses a shared
synchronization coordinator that
bypasses cache TTLs, prevents overlapping work per source, and continues when an
individual repository fails. Both editions also invoke the same coordinator on
a 60-second background schedule, and the UI observes completed synchronization
runs through lightweight status polling. Connections and discovery are driven
through the source-module registry in both editions. Every non-health server API
requires an authenticated web session.

```sh
npm install
npm run contracts:generate
npm run check
```

The canonical logo is `assets/opsscope-logo.png`. After changing it, run
`npm run icons:generate` to regenerate the web, desktop, and packaging icons.

For the browser UI, run `npm run server:dev` and `npm run web:dev` in separate
terminals. Run the desktop shell with `npm run desktop:dev`; the Tauri CLI is
installed locally by npm. See the [development guide](docs/development.md) for
the complete command and dependency-boundary reference.
