# Hermes Agent — Client App

End-user product for Cloud Agent usage.

## Responsibility

- User login/register/OAuth (self-hosted auth via `/api/v1/auth/*`)
- Cloud Agent session list/create/delete
- Cloud Agent chat, streaming, interrupt
- Commit list and git-policy management

`apps/web-app` is user-facing.  
`apps/dashboard` is admin/ops-facing.

## Development

```bash
cd apps/web-app
pnpm install
pnpm dev:web
```

Desktop shell in the same project:

```bash
pnpm dev:desktop
```

To connect to a remote backend, set:

```bash
VITE_API_BASE_URL=http://127.0.0.1:8787
```

