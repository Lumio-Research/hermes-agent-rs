# Hermes Deployment Assets

Current recommended path for this repository is **Fly.io-first tenant runtime deployment**, not Kubernetes-first rollout.

## What was cleaned

- Removed `control-plane` chart (K8s-oriented).
- Removed `shared-observability` chart (Prometheus Operator-oriented).

These were useful for architecture exploration, but are not the current MVP path.

## What remains

- `tenant-runtime/`: optional K8s reference chart kept for future backend support.
- `fly/`: active templates for Fly.io tenant runtime provisioning.

## Recommended flow now

1. Define tenant runtime with `fly/tenant-spec.example.yaml`.
2. Render `fly/fly.toml.tmpl` with tenant values.
3. Provision app/volume/secrets via `fly/provision-tenant.sh`.
4. Deploy tenant runtime to Fly.io.

## Minimal automation assets

- `fly/render-fly-toml.sh`: render `fly.toml` from normalized tenant values.
- `fly/provision-tenant.sh`: create app/volume, set secrets, deploy, optional domain bind.
- `fly/spec-to-provision.ts`: TypeScript converter from tenant spec to provision args/command.
- `fly/control-plane-deploy.ts`: stateful deploy runner with lock, health gate, rollback and persisted status (`file`/`postgres` backend, `env`/`file`/`vault` secrets backend).
- `fly/CONTROL_PLANE_AUTOPROVISION.md`: control-plane invocation contract and state model.
- `fly/PRODUCTION_READINESS.md`: production readiness checklist and hardening gaps.
- `fly/secrets.example.env`: local example of secret injection format.

## Notes

- Keep secrets out of spec files. Use secret references only.
- Use one Fly app per tenant for strong isolation.