# OpsScope

<p align="center">
  <img src="assets/opsscope-logo.png" alt="OpsScope logo" width="128" height="128">
</p>

An operational dashboard for your repositories: see failing workflows, pull
requests that need attention, and open issues in one place. Run it on your
desktop or host it yourself.

## Why OpsScope?

Following work across repositories means switching between Actions pages,
pull request lists, and issue trackers just to answer “what needs my attention?”
OpsScope brings those signals together in a compact, project-grouped view.

It complements your source platform rather than replacing it. Inspect status,
activity, and run logs here; open the original page when you need to act.
Credentials and cached data stay on your machine or server, with no
OpsScope-hosted service required.

GitHub.com and GitHub Enterprise Server are supported today. The shared core
and source-module architecture allow other providers and kinds of operational
information to be added without building a separate application for each.

## Features

- **Workflows:** latest and previous runs, status filters, duration, and
  on-demand logs in a dialog.
- **Pull requests:** open PRs grouped by repository, with reviews and checks;
  supported workflow checks open run details directly.
- **Issues:** open issues grouped by repository, with details and discussion.
- **Activity:** a combined feed of workflow runs and observed PR changes.
- **Background monitoring:** scheduled and manual refresh, cached results on
  reload, and freshness information when a source is unavailable.
- **Notifications:** workflow failures, PR updates, and issue updates through
  native desktop notifications or Apprise, with event controls in Settings.
- **Personal scope:** optionally show only work relevant to each connection's
  token owner, including notification filtering.

Both editions share the same core and UI. Each installation is intended for a
single user: there are no separate workspaces or per-user data permissions.
Integrations are read-only; rerunning jobs, editing issues, and merging PRs are
not supported. GitHub is currently the only source module.

## Quick start

### Desktop

Download your platform's asset from
[Releases](https://github.com/dougmaitelli/OpsScope/releases).

| Platform | Download and run |
| --- | --- |
| macOS (Apple Silicon and Intel) | Extract the universal `.app.zip`, move OpsScope.app to Applications, and open it. |
| Windows x86-64 | Run the portable `.exe`. |
| Linux x86-64 | Make the `.AppImage` executable and run it, or use the raw binary with its system dependencies installed. |

Desktop releases are not currently signed with a trusted publisher certificate
or notarized by Apple; platform security warnings may appear. The raw Linux
binary needs the WebKitGTK 4.1 and tray libraries used by the desktop shell;
it is not a standalone static executable.

Minimize the window to keep monitoring from the system tray. Use the tray menu
to restore it or quit. Desktop operation does not require the web server.

### Self-hosted with Docker

Save this as `compose.yaml`:

```yaml
services:
  opsscope:
    image: ghcr.io/dougmaitelli/opsscope:latest
    restart: unless-stopped
    ports:
      - "127.0.0.1:4317:4317"
    volumes:
      - opsscope-data:/data

volumes:
  opsscope-data:
```

Run `docker compose up -d`, then open **http://127.0.0.1:4317** and connect
a source below.
The published image targets **linux/amd64**.

This example exposes the service only on the Docker host's loopback interface.
**Authentication is disabled unless OIDC is configured.** Do not expose an
unauthenticated instance to an untrusted network.

### Connect your repositories

1. Open **Connections**, add a GitHub connection, and provide a personal access
   token. Keep `https://github.com` or enter your GitHub Enterprise Server
   origin, such as `https://github.example.com`.
2. In **Repositories**, select the repositories to monitor.
3. Open **Workflows**, **Pull requests**, **Issues**, or **Activity**. Use Refresh
   for an immediate update; synchronization also runs in the background.
4. Adjust refresh frequency, history, or personal scope in **Settings**.

Multiple GitHub servers are supported, but duplicate connections to the same
normalized server URL are not. Replace a connection's token from Connections
when it expires or needs different permissions.

For a fine-grained token, grant access to the selected repositories and read
access to **Metadata, Actions, Pull requests, Issues, Checks, and Commit
statuses**. A classic token with **repo** scope is also supported; it grants
broader permissions than OpsScope uses. Organization token approval and access
policies still apply.

## Deployment

The image serves the UI and API, runs as an
unprivileged user, and has a health check at `/api/health`. A bind-mounted data
directory must be writable by the container user.

For remote access, put an HTTPS reverse proxy in front of the service and
configure OIDC below, or provide an equivalent trusted access boundary.
Proxy the entire site, including `/api`, at the root of its public origin.
A proxy in another container should reach `opsscope:4317` over a shared Docker
network; the host's loopback mapping is not that container's loopback.

Use a version tag such as `:v0.3.1` instead of `:latest` for controlled upgrades.

### Server configuration

Set these environment variables on the server process or in the Compose
service's `environment` section. They are not desktop settings.

| Variable | Default | Purpose |
| --- | --- | --- |
| `OPSSCOPE_BIND_ADDRESS` | `127.0.0.1:4317`; container: `0.0.0.0:4317` | HTTP listening address. |
| `OPSSCOPE_DATA_DIR` | `.opsscope-data`; container: `/data` | SQLite database and default encryption-key directory. |
| `OPSSCOPE_MASTER_KEY_FILE` | `<data directory>/master.key` | Raw 32-byte credential-encryption key file; created if absent. |
| `OPSSCOPE_APPRISE_URL` | Unset (delivery disabled) | Apprise HTTP(S) notification endpoint. |
| `OPSSCOPE_APPRISE_TAGS` | Unset (no tag filter) | Comma-separated Apprise routing tags. |
| `OPSSCOPE_PUBLIC_URL` | Unset | Browser-visible origin, required with OIDC. |
| `OPSSCOPE_OIDC_ISSUER` | Unset | OIDC discovery issuer URL. |
| `OPSSCOPE_OIDC_CLIENT_ID` | Unset | Confidential OIDC client ID. |
| `OPSSCOPE_OIDC_CLIENT_SECRET` | Unset | Confidential OIDC client secret. |
| `OPSSCOPE_OIDC_ALLOWED_SUBJECTS` | Unset | Optional comma-separated allowed `sub` values. |

### Optional OIDC authentication

Register a confidential web client with your identity provider using this
redirect URI (substitute your public origin):

```text
https://ops.example.com/api/auth/callback
```

Add this to the Compose service:

```yaml
    environment:
      OPSSCOPE_PUBLIC_URL: https://ops.example.com
      OPSSCOPE_OIDC_ISSUER: https://identity.example.com/your-issuer
      OPSSCOPE_OIDC_CLIENT_ID: opsscope
      OPSSCOPE_OIDC_CLIENT_SECRET: ${OPSSCOPE_OIDC_CLIENT_SECRET:?Set the OIDC client secret}
      OPSSCOPE_OIDC_ALLOWED_SUBJECTS: your-subject-id
```

Supply the secret through your deployment environment; do not commit it.
OpsScope uses authorization-code flow with PKCE. The public URL must be an
origin without a path, and HTTPS is required except on loopback.

OIDC is disabled when issuer, client ID, and client secret are all unset or
blank. Setting any one requires all three plus the public URL; incomplete
configuration or failed discovery prevents startup. Public URL or allowed
subjects alone do not enable authentication.

Without an allowed-subject list, restrict admission through the identity
provider's client policy. OIDC controls access to the installation, not
individual connections: every admitted user shares the same data and
credentials. The GitHub token owner—not the OIDC login—defines “me.”

### Apprise notifications

Set `OPSSCOPE_APPRISE_URL` to a reachable Apprise API notification endpoint,
for example `http://apprise:8000/notify/opsscope`. Configure destinations and
their credentials in Apprise, not in OpsScope. OpsScope sends a notification
payload to that endpoint; it does not run Apprise for you.
Leave the variable unset to disable delivery.

To route notifications to tagged destinations in your Apprise configuration,
set `OPSSCOPE_APPRISE_TAGS`. For example, in the Compose service:

```yaml
    environment:
      OPSSCOPE_APPRISE_URL: http://apprise:8000/notify/opsscope
      OPSSCOPE_APPRISE_TAGS: opsscope,alerts
```

Whitespace around tags and empty entries are ignored. When unset or blank,
OpsScope sends no tag filter, preserving the endpoint's default routing.
Tags alone do not enable notifications; the endpoint must also be configured.

Choose notification events in **Settings → Notifications**:

| Area | Events |
| --- | --- |
| Workflows | Actual latest run fails (not historical failures). |
| Pull requests | New PR, review requested from you, changes requested, merged, or closed. |
| Issues | New issue, assigned to you, reopened, or closed. |

All event types are enabled by default. “You” is the connection's token owner.
**Only show my work** applies to every event, including assignment alerts.
Comments and successful workflow runs do not trigger notifications.

The first successful observation establishes a baseline without notifying about
existing items. Subsequent changes are grouped per repository: PR and issue
events share one message with titles and links; workflow failures use a separate
message. Observations persist across restarts, and disabled or filtered events
are still recorded so enabling them does not replay old changes.

Polling detects changes observed between refreshes, not every intermediate event.
Closures require a successful detail lookup; disappearance from an open-items list
alone is not treated as a closure. Reopens can be identified for issues previously
observed as closed. Notifications are recorded before delivery to prevent duplicates;
a failed delivery is logged but not automatically retried.

### Persistence, backups, and upgrades

Desktop metadata lives in the application's OS data directory, and tokens live
in the OS keychain. The server stores metadata and encrypted tokens in
`opsscope.sqlite3`, using `master.key` by default.

Persist the entire server data directory. For a simple consistent backup,
stop the container and back up the volume **and its encryption key** before
restarting it. Back up a separately mounted key too.
Losing the key makes stored tokens unreadable; replacing it is not a key
rotation procedure.

For stronger separation, mount a raw 32-byte key file and set
`OPSSCOPE_MASTER_KEY_FILE` to its path. It must be readable by the server user
and, on Unix, grant no group or world permissions. A separate mount is not
enforced: the generated default key works in deployed containers too.
Encryption protects a database-only disclosure, not a compromised host or a
backup containing both the database and key.

After backing up, update your image tag if pinned, then run:

```sh
docker compose pull
docker compose up -d
```

Keep the same volume and key. Database migrations run at startup.
Restarting the server also invalidates existing OIDC sessions.

## In-app settings

| Setting | Default | Behavior |
| --- | --- | --- |
| Synchronization interval | 60 seconds | Background refresh, configurable from 30 to 3,600 seconds. |
| Recent runs per workflow | 10 | Recent-run window, configurable from 1 to 100. |
| Only show my work | Off | Filters monitoring views and notifications using each connection's token owner. |
| Notification events | All enabled | Individual switches for workflow, PR, and issue notifications. |
| Pull request monitoring | On | Enables the PR page, PR activity, synchronization, and PR notifications. |
| Issue monitoring | On | Enables the Issues page, synchronization, and issue notifications. |

Disabling a feature hides its navigation entry and stops its monitoring requests
and notifications. Cached data and notification preferences are preserved.
Re-enabling establishes a fresh notification baseline, without replaying changes
from the disabled period. Workflows remain available independently.

Personal scope includes PRs you authored, reviewed, or were asked to review;
issues you authored, subscribed to, or participated in; and workflow runs
associated with your commits or PRs.

The displayed latest matching workflow run may not be the repository's actual
latest run. Notifications still use the actual latest run. Relevance is
determined within fetched data, not by searching unlimited history.
Items whose relevance cannot be determined are hidden in personal scope;
refresh to populate missing metadata. Turning it off restores the full cached
view without reconnecting sources.

## Development and design

For local setup, commands, and release maintenance, see the
[development guide](docs/development.md). The [architecture](docs/architecture.md)
explains the shared core and source modules; the
[security model](docs/security.md) describes trust boundaries and limitations.
