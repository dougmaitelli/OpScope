# CI Watcher

CI Watcher is a local-first application for monitoring software-delivery
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

- One GitHub connection using a fine-grained personal access token.
- Repository and GitHub Actions workflow discovery.
- Current and recent workflow-run state across selected repositories.
- Repository, workflow, branch, commit, actor, trigger, timing, outcome,
  freshness, and links back to GitHub.
- Manual and scheduled refresh with cached state during transient failures.
- Search and filtering by repository, workflow, and status.
- Configurable local history stored in SQLite.
- Native desktop and single-administrator self-hosted editions.

It does not include write actions, workflow logs or artifacts, notifications,
GitHub Enterprise Server, multiple GitHub connections, multiple users,
PostgreSQL, other providers, runtime plugins, mobile applications, or a hosted
service.

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
GitHub account validation, credential persistence, repository selection,
workflow discovery, and recent workflow-run monitoring are implemented. Run
activity and synchronization freshness are cached in SQLite, with the last
successful snapshot retained during transient provider failures. Desktop
credentials use the operating-system keychain. The server encrypts credentials
stored in SQLite. Connections and discovery are driven through the source-module
registry in both editions. The unauthenticated development server binds only to
`127.0.0.1`.

```sh
npm install
npm run contracts:generate
npm run check
```

For the browser UI, run `npm run server:dev` and `npm run web:dev` in separate
terminals. Run the desktop shell with `npm run desktop:dev`; the Tauri CLI is
installed locally by npm. See the [development guide](docs/development.md) for
the complete command and dependency-boundary reference.
