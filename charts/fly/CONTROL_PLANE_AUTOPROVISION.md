# Control Plane Auto-Provision Contract (Fly.io)

This document defines a minimal contract for control-plane automation.

## Goal

Control-plane takes one tenant runtime spec and automatically provisions a tenant-isolated Fly app.

## Inputs

- Tenant spec (recommended shape: `tenant-spec.example.yaml`)
- Secret material (from secure store, not from spec)

## Required normalized payload

Before invoking shell automation, control-plane should normalize spec into:

- `app_name`
- `region`
- `image`
- `tenant_id`
- `tenant_name`
- `environment`
- `log_level`
- `hermes_home`
- `public_base_url`
- `volume_name`
- `volume_size_gb`
- `internal_port`
- `min_machines`
- `vm_size`
- `domain` (optional)
- `secrets_file` (temp file path or generated env pairs)

## Provisioning command

```bash
charts/fly/provision-tenant.sh \
  --template charts/fly/fly.toml.tmpl \
  --app-name "$APP_NAME" \
  --region "$REGION" \
  --image "$IMAGE" \
  --tenant-id "$TENANT_ID" \
  --tenant-name "$TENANT_NAME" \
  --environment "$ENVIRONMENT" \
  --log-level "$LOG_LEVEL" \
  --hermes-home "$HERMES_HOME" \
  --public-base-url "$PUBLIC_BASE_URL" \
  --volume-name "$VOLUME_NAME" \
  --volume-size-gb "$VOLUME_SIZE_GB" \
  --internal-port "$INTERNAL_PORT" \
  --min-machines "$MIN_MACHINES" \
  --vm-size "$VM_SIZE" \
  --secrets-file "$SECRETS_FILE" \
  --domain "$DOMAIN" \
  --output-dir ".fly-generated/$TENANT_ID"
```

## TypeScript converter (recommended)

Use `spec-to-provision.ts` to convert one tenant spec into normalized provision arguments.

Install deps:

```bash
cd charts/fly
pnpm install
```

Print shell command:

```bash
pnpm tsx spec-to-provision.ts \
  --spec tenant-spec.example.yaml \
  --secrets-file /tmp/acme.secrets.env
```

Print JSON payload:

```bash
pnpm tsx spec-to-provision.ts \
  --spec tenant-spec.example.yaml \
  --secrets-file /tmp/acme.secrets.env \
  --format json
```

Directly execute provisioning:

```bash
pnpm tsx spec-to-provision.ts \
  --spec tenant-spec.example.yaml \
  --secrets-file /tmp/acme.secrets.env \
  --exec
```

## Control-plane deploy runner (production path)

Use `control-plane-deploy.ts` for stateful deploy workflow with lock, health gate, rollback, and persistent events.

```bash
cd charts/fly
pnpm install
```

Deploy with env-backed secret manager:

```bash
export OPENAI_API_KEY=...
export TELEGRAM_BOT_TOKEN=...
export WEB_WIDGET_SECRET=...
export ORDER_API_KEY=...

pnpm exec tsx control-plane-deploy.ts \
  --action deploy \
  --spec tenant-spec.example.yaml \
  --template fly.toml.tmpl \
  --secrets-backend env
```

Deploy with Vault KVv2 backend:

```bash
pnpm exec tsx control-plane-deploy.ts \
  --action deploy \
  --spec tenant-spec.example.yaml \
  --template fly.toml.tmpl \
  --secrets-backend vault \
  --vault-address https://vault.example.com \
  --vault-token "$VAULT_TOKEN" \
  --vault-mount secret \
  --vault-path-prefix hermes/tenants
```

Deploy with file backend (temporary bootstrap only):

```bash
pnpm exec tsx control-plane-deploy.ts \
  --action deploy \
  --spec tenant-spec.example.yaml \
  --template fly.toml.tmpl \
  --secrets-backend file \
  --secrets-file /tmp/acme.secrets.env
```

Persist state/events in Postgres:

```bash
pnpm exec tsx control-plane-deploy.ts \
  --action deploy \
  --spec tenant-spec.example.yaml \
  --template fly.toml.tmpl \
  --state-backend postgres \
  --pg-connection-string "$CONTROL_PLANE_DB_URL" \
  --pg-schema hermes_control_plane \
  --secrets-backend vault \
  --vault-address https://vault.example.com \
  --vault-token "$VAULT_TOKEN"
```

Manual one-click rollback to last stable image:

```bash
pnpm exec tsx control-plane-deploy.ts \
  --action rollback \
  --spec tenant-spec.example.yaml \
  --template fly.toml.tmpl
```

Query persisted status/events:

```bash
pnpm exec tsx control-plane-deploy.ts \
  --action status \
  --spec tenant-spec.example.yaml
```

Skip secrets completeness validation (not recommended):

```bash
pnpm tsx spec-to-provision.ts \
  --spec tenant-spec.example.yaml \
  --secrets-file /tmp/acme.secrets.env \
  --no-secret-validation
```

## Suggested API surface

- `POST /tenants` -> create tenant + draft spec
- `POST /tenants/{tenantId}/deploy` -> run provisioning
- `POST /tenants/{tenantId}/redeploy` -> redeploy image/config
- `GET /tenants/{tenantId}/status` -> fetch tenant runtime state
- `GET /tenants/{tenantId}/events` -> deployment event log

## Suggested runtime states

- `draft`
- `validating`
- `provisioning`
- `deploying`
- `verifying`
- `running`
- `rollback_in_progress`
- `rolled_back`
- `degraded`
- `failed`

## Security notes

- Keep plaintext secrets out of Git and tenant spec.
- Prefer Vault/managed secret manager in control-plane (avoid file backend outside bootstrap).
- Use least-privilege Fly API token for automation.
- `spec-to-provision.ts` validates that required `*SecretRef` keys exist in secrets file by default.

