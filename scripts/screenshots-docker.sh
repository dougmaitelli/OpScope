#!/bin/sh
set -eu
exec sh "$(dirname "$0")/e2e-docker.sh" --config playwright.docs.config.ts "$@"
