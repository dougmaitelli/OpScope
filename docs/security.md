# Security model

OpsScope handles tokens that may read private repositories. It is designed for
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

## Provider credentials

Tokens are accepted through connection input and are not returned in response
DTOs. Prefer narrowly scoped, expiring fine-grained tokens. Classic tokens are
supported but can grant write permissions that OpsScope does not use.

Server tokens use XChaCha20-Poly1305 with random data-encryption keys wrapped by
a separate master key. OIDC secrets are not used for credential encryption.
Secret values are zeroized where practical. Credential replacement and deletion
produce audit events.

The server loads or creates a raw 32-byte master-key file. On Unix, existing key
files with group or world permissions are rejected. A separate mounted key is
recommended for separation from database storage, but is not mandatory.

Encryption protects a database-only disclosure. A backup containing both the
database and key can decrypt the tokens. Keep both recoverable and protect
them accordingly; deleting or replacing a key is not supported key rotation.

## Provider requests

GitHub connection configuration accepts a server origin rather than an arbitrary
API path. Remote origins require HTTPS; loopback HTTP is accepted for local
development. Credentials, non-root paths, query strings, and fragments in
origins are rejected.

Authenticated GitHub API requests do not follow redirects. Private-network
origins are allowed for Enterprise Server, so connection configuration must be
treated as a trusted-administrator capability, not exposed to untrusted users.

## Release and operational limitations

CI pins actions to immutable revisions, uses committed dependency lockfiles,
and runs checks and tests. Container publishing enables provenance and SBOM
generation. Desktop builds are not signed with trusted publisher certificates
or notarized by Apple; macOS uses ad-hoc signing.

Back up the database and encryption key before upgrading. Cached data supports
continued viewing during provider failures, but is not a substitute for
provider backups or a complete historical audit record.

The current application does not implement GitHub webhooks, multi-user
authorization, or automatic credential-key rotation. Any future addition must
define its own authentication and secret lifecycle rather than reusing OIDC
or storage keys.
