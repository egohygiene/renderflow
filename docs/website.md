# Renderflow product website

The standalone Renderflow product site lives in `apps/web` and is designed for deployment beneath `/renderflow/`.

## Local development

Requirements:

- Node `24.x`
- Corepack-enabled `pnpm`

Install and run:

```bash
corepack enable
pnpm install --frozen-lockfile
pnpm web:dev
```

Local URL:

```text
http://localhost:5173/renderflow/
```

## Commands

```bash
pnpm web:build
pnpm web:preview
pnpm web:format
pnpm web:lint
pnpm web:typecheck
pnpm web:test
pnpm web:check
```

## Deployment contract

- build command: `pnpm web:build`
- output directory: `apps/web/dist`
- preview command: `pnpm web:preview`
- public base path: `/renderflow/`
- environment variables: `VITE_PUBLIC_BASE_PATH` (optional override, defaults to `/renderflow/`)
- smoke-check path: `/renderflow/`
- gateway ownership: `egohygiene.io/renderflow/*`
- SPA fallback: serve `apps/web/dist/index.html` or `apps/web/dist/404.html` for unknown `/renderflow/*` routes

## Build notes

- Vite owns the canonical public base setting and the router basename derives from the same value.
- Built assets resolve under `/renderflow/assets/`.
- The product-site manifest is `apps/web/site-manifest.json` and acts as the integration contract for the Ego Hygiene gateway.
- The initial site is intentionally static and lightweight; keep meaningful bundle regressions under review rather than treating an arbitrary score as a release gate.
- The product website explains Renderflow and links back to canonical technical documentation instead of duplicating the full reference docs.
