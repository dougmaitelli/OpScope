#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
PLAYWRIGHT_VERSION=$(node -p 'require("./package.json").devDependencies["@playwright/test"]')
case "$PLAYWRIGHT_VERSION" in
  *[!0-9.]*|'') echo 'Pin @playwright/test to an exact version' >&2; exit 1 ;;
esac
docker build --build-arg "PLAYWRIGHT_VERSION=$PLAYWRIGHT_VERSION" -f e2e/Dockerfile -t opsscope-e2e .
docker run --rm --init --ipc=host \
  -e CI=true -e HOME=/tmp \
  --user "$(id -u):$(id -g)" \
  -v "$PWD:/work" -w /work \
  --tmpfs /work/node_modules:exec,mode=1777 \
  opsscope-e2e \
  sh -c 'npm ci && npm run typecheck:e2e && npm run test:e2e -- "$@"' sh "$@"
