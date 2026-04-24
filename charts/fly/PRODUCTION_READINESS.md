# Fly Tenant Runtime - Production Readiness

This checklist tracks what is already implemented and what still needs to be completed for production use.

## Already in place

- Tenant spec example and mapping docs.
- TypeScript converter (`spec-to-provision.ts`) with strict field validation:
  - app/domain format checks
  - URL checks
  - port/size/range checks
  - required secret reference checks against secrets file
- Provision script (`provision-tenant.sh`) with:
  - Fly auth precheck
  - secrets format and placeholder validation
  - idempotent app/volume creation
  - deploy retry
- Stateful deploy runner (`control-plane-deploy.ts`) with:
  - secret backend modes (`env`, `file`, `vault`)
  - persistent state/event backend modes (`file`, `postgres`)
  - per-tenant deployment lock
  - health gate on `/healthz` and `/readyz`
  - automatic rollback to last stable image (can be disabled)
  - manual rollback action

## Required before production

- Add CI checks:
  - `pnpm -C charts/fly install`
  - `pnpm -C charts/fly spec:help`
  - `pnpm -C charts/fly cp:help`
  - static lint for shell scripts
- Add audit trail:
  - who deployed
  - spec diff
  - deployed image digest

## Recommended next hardening

- Use Fly API token scoped per environment.
- Enforce region allow-list in control-plane.
- Use postgres backend in production for multi-instance control-plane.
- Use vault backend in production for secret retrieval and rotation.
- Add DNS readiness checks after custom domain bind.
