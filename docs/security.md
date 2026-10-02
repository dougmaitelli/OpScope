# Security model

OpScope handles tokens that may read private repositories. It is designed for
one trusted user per installation, not multi-tenant isolation.
Deployment instructions and environment variables are documented in the
[README](../README.md#deployment).

## Trust boundaries

- Desktop trusts the OS user and native process. The webview receives data
  through allowlisted IPC commands; credentials are stored in the OS keychain.
- The server process can decrypt provider credentials and must be trusted.
  Every admitted browser session can operate the same installation.
- Provider content is untrusted. The frontend renders provider text without
  treating it as application code.
- A compromised host, native process, or authenticated frontend session is
  outside the protection offered by credential encryption.

## Optional web authentication

With OIDC enabled, the server uses authorization-code flow with PKCE, state,
and nonce validation. Sessions use opaque identifiers, with only their hashes
retained in server memory. Restarting the server invalidates them.
Production session cookies are HttpOnly, Secure, and SameSite=Strict.
Authenticated mutations require CSRF validation.

Pending OIDC logins are capped at 64 per server process, with a global limit of
10 new login attempts per minute. Excess attempts receive HTTP 429 with
`Retry-After`; they do not evict active login attempts or invalidate sessions.
Session lookup does not scan the pending-login collection. Operators exposing
OIDC publicly should also apply proxy-level rate limiting for broader traffic
protection.

OIDC is disabled when issuer, client ID, and client secret are all absent or
blank. Public URL and allowed subjects alone do not enable it. In disabled mode,
the UI and API require neither a session nor a CSRF token. Restrict network
access or provide authentication at another trusted boundary.

The standalone server defaults to loopback; the container listens on all
container interfaces. Published ports and proxy configuration determine actual
network exposure. Partial OIDC configuration fails startup rather than falling
back to unauthenticated operation.

An optional subject allowlist restricts admission. Without it, provider-side
client policy must restrict access. OIDC identity does not change source
permissions or the token-owner identity used for personal scope.
