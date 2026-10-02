ARG PLAYWRIGHT_VERSION
FROM mcr.microsoft.com/playwright:v${PLAYWRIGHT_VERSION}-noble
WORKDIR /work
ENV CI=true
CMD ["sh", "-c", "npm ci && npm run typecheck:e2e && npm run test:e2e"]
