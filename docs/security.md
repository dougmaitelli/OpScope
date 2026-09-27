# Security and threat model

## Security objectives

CI Watcher handles credentials capable of reading private repository metadata.
Future versions may hold credentials with write permissions. Its design must:

- Accept new provider credentials only through intentional input, never return
  them to the frontend, and keep them out of routine logs.
- Authenticate every non-public self-hosted API route.
- Prevent one security credential from serving unrelated purposes.
- Minimize provider permissions and repository scope.
- Make insecure network exposure an explicit development-only choice.
- Preserve an audit trail for credential and configuration changes.
- Bound the impact of a compromised webview or browser session.

## Trust boundaries

### Desktop

- The operating-system user and native core process are trusted.
- The webview is less trusted and receives only presentation data.
- A narrow, allowlisted IPC command surface forms a privilege boundary.
- The operating-system keychain is the provider credential store.

### Self-hosted

- The browser is untrusted until a server-side session is authenticated.
- The reverse proxy and forwarded headers are untrusted unless explicitly
  configured as trusted.
- The application process can decrypt provider credentials and is sensitive.
- SQLite ciphertext, database backups, logs, and container metadata may be
  copied independently and need different protections.
- GitHub webhook requests, when introduced, are untrusted until their signature
  and delivery identity are verified.

## Principal threats and controls

### Deployment without authentication

OIDC is optional. When no issuer, client ID, or client secret is present, the API is
available without sessions or CSRF checks. The server itself remains bound to
loopback; an operator exposing it through a reverse proxy in this mode must
provide an equivalent trusted access boundary. Partially configured OIDC fails
startup instead of silently disabling authentication.

### Authentication credential disclosure

- The self-hosted edition uses the OIDC authorization-code flow with PKCE,
  state, and nonce validation.
- Provider access, refresh, and ID tokens are not returned to the browser or
  retained after authentication.
- Sessions use random opaque identifiers stored server-side.
- Only a hash of a session identifier is retained in process memory; restarting
  the server invalidates existing sessions.
- Cookies are `HttpOnly`, `Secure`, and `SameSite=Strict` in production.
- Authentication responses and logs never contain reusable credentials.

### Cross-site request forgery

All authenticated state-changing HTTP requests require both a same-site session
and CSRF validation. Login and other pre-authentication submissions require
strict origin validation and login-CSRF protection. Read endpoints must not
cause state changes.

### Cross-site scripting and frontend compromise

- Apply a restrictive Content Security Policy.
- Do not load application scripts from third-party CDNs.
- Treat provider-controlled text as untrusted and render it as text.
- Do not expose stored secrets through response DTOs, DOM attributes, URLs, or
  browser storage. Clear credential inputs after submission.
- Desktop capabilities and commands are allowlisted narrowly.
- The desktop webview cannot navigate to remote content with native privileges.

### Provider credential theft

- Request read-only, repository-limited GitHub permission for version one.
- Recommend fine-grained tokens with expiration.
- Desktop credentials live in the OS keychain.
- Server credentials are encrypted with a random data-encryption key.
- The server master key is supplied through a mounted secret file or equivalent
  secret manager integration, never the OIDC client secret.
- Secret values are zeroized where practical and excluded from debug output.
- Credential replacement and deletion create audit events.

The current loopback-only development server creates a local master-key file
when none is configured. Each provider token is encrypted with XChaCha20-
Poly1305 using a random data-encryption key; that key is separately wrapped by
the server master key. Production server startup will require a separately
mounted key file. Existing key files with group or world permissions are
rejected on Unix platforms.

Application encryption protects against a database-only disclosure. It does not
claim to protect credentials after complete compromise of the running host.

### Server-side request forgery

GitHub connections accept a server origin, not an arbitrary API endpoint.
Remote origins require HTTPS; loopback HTTP is accepted only for development.
Origins containing credentials, paths, query strings, or fragments are rejected
and normalized before duplicate detection. Authenticated API requests never
follow redirects, which prevents a configured server from redirecting a token
to another destination. Private-network origins remain available because they
are a normal GitHub Enterprise deployment target and must therefore be treated
as administrator-controlled configuration.

### Synchronization abuse and provider rate limits

- Apply minimum refresh intervals and prevent overlapping jobs.
- Honor provider retry and rate-limit response information.
- Use bounded retries and jitter.
- Limit pagination, response sizes, concurrency, and retained history.
- Do not allow browser requests to proxy arbitrary GitHub API operations.

### Database and migration failure

- Use transactions for projection updates.
- Run migrations before background work begins.
- Back up before destructive migrations.
- Test upgrade and restore paths.
- Never silently discard incompatible provider details.

### Supply-chain and release compromise

- Commit lockfiles.
- Review and audit native-core and JavaScript dependencies.
- Pin CI actions to immutable revisions.
- Enable repository secret scanning and dependency update automation.
- Produce a software bill of materials for releases.
- Sign desktop installers and update artifacts.
- Keep release signing keys outside ordinary development environments.

## Self-hosted identity configuration

OIDC issuer, client ID, client secret, and public origin are supplied through
the deployment environment. If none are present, OIDC is disabled. If any OIDC
setting is present, startup fails when required values are missing, discovery
fails, or a non-loopback public origin does not use HTTPS. An optional subject
allowlist provides application-side admission control; otherwise the
administrator must restrict access through the provider's client policy.

## Future GitHub App and webhooks

When GitHub App support is introduced:

- Installation tokens are used for unattended observation.
- User access tokens are required for actions attributable to a person.
- Tokens are cached only until their expiry and can be revoked.
- Webhook signatures are compared in constant time.
- Delivery IDs provide replay and idempotency protection.
- Only required events are subscribed to.
- App private keys are never shipped in the desktop application.

## Security acceptance criteria for version one

- A test proves that serialized frontend DTOs cannot contain secret fields.
- A test proves every protected HTTP route rejects an anonymous request.
- State-changing route tests cover missing and invalid CSRF tokens.
- Logs are tested for redaction using representative provider failures.
- Partial or invalid OIDC configuration fails closed.
- Desktop IPC and capability configuration grants no unused native permission.
- Backup files do not contain plaintext GitHub credentials.
- Authentication and encryption secrets can be rotated independently.
