# Browser integration and visual regression tests

This suite runs the production React build in Chromium. Playwright intercepts
the HTTP API using typed, deterministic fixtures; it exercises the real UI,
router, HTTP client, browser layout, and dialogs. It does not run the Rust server,
provider adapters, OIDC identity provider, or native Tauri shell. Rust server and
adapter tests cover those boundaries separately.

## Run and inspect failures

```sh
# Canonical Linux browser environment, also used in CI
npm run test:e2e:docker
npm run test:e2e:docker -- --project=desktop --grep 'workflow filtering'

# Local debugging (requires Chromium)
npx playwright install chromium
npm run test:e2e
npm run test:e2e:ui
npx playwright show-report
```

Docker derives the browser image from the exact @playwright/test dependency pin.
It installs locked dependencies on an isolated executable temporary mount, without
changing host node_modules. Both runners build the frontend and start an isolated
preview server on port 1430; another process on that port causes a failure.
Run screenshot export and regression tests sequentially.

## Intentional UI changes

1. Run the canonical tests and inspect behavior assertions and image differences.
2. Update affected baselines in the canonical environment:

   ```sh
   npm run test:e2e:docker -- --update-snapshots --grep 'Workflows layout'
   ```

3. Review and commit the changed PNGs under e2e/snapshots alongside the UI change.
4. Run the tests again without the update flag.

Normal runs never create or update baselines. npm run test:e2e:update is the
direct local update task; canonical Docker updates keep rendering consistent
with CI. Review baseline changes again after upgrading Playwright.

## Coverage and deterministic inputs

The desktop viewport (1400×900) covers Workflows, Pull requests,
Issues, Activity, Settings, workflow logs, PR and issue dialogs, and service errors.
Behavior tests cover navigation, filtering and URL reload, refresh request scope,
discussion rendering, feature settings save/reload, service recovery, action
availability, confirmation cancellation, and workflow rerun submission. Dialog
baselines include enabled and disabled action buttons and confirmation popovers.

Each test has a fresh browser context and its own fixture state. Fixtures use
generated contracts, fixed dates, locale, timezone, identifiers and data. Tests
fail on browser exceptions and unexpected API or external requests. Screenshots
wait for meaningful content, fonts and images, with animations disabled and zero
differing pixels (Playwright's default per-pixel color threshold). No arbitrary
sleeps or whole-region masks hide differences.

Add focused interactions and snapshots when extending the UI. Fixtures should
model new API contracts explicitly. Passing this suite does not establish backend
persistence, real-provider integration, accessibility, or cross-browser support.

## Documentation screenshots

```sh
npm run screenshots:docker
# Or locally with installed Chromium
npm run screenshots
```

scripts/take-screenshots.ts shares fixtures, screen helpers, and rendering setup
with the regression suite. It exports the four monitoring images referenced in the
README to screenshots/, with a macOS-style window frame and transparent padding.
The frame styling lives in scripts/screenshot-frame.css. Review and commit the
images when the documented UI changes.
Documentation export never updates regression baselines; baseline updates never
export documentation images. CI uploads the HTML report, image differences,
traces and failure videos as ui-test-report.
