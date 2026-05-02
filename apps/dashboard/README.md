# Hermes Agent — Admin Dashboard

Browser-based admin dashboard for operators and maintainers. It is used for platform management (status, logs, sessions, config, analytics, cron, skills, keys).

Cloud Agent end-user chat/create flows are intentionally hosted in `apps/web-app`, not in this dashboard.

## Stack

- **Vite** + **React 19** + **TypeScript**
- **Tailwind CSS v4** with custom dark theme
- **shadcn/ui**-style components (hand-rolled, no CLI dependency)

## Development

```bash
# Rust binary: start the API server (default http://127.0.0.1:8787; see vite.config.ts)
cd ../
hermes serve --no-gateway --no-cron --port 8787

# In another terminal, start the Vite dev server (with HMR + API proxy)
cd apps/dashboard/
npm run dev
```

The Vite dev server proxies `/api` to `HERMES_SERVER_URL` or `http://127.0.0.1:8787` by default.

If you use the **Python** Hermes stack instead, start its web UI command and point `HERMES_SERVER_URL` at that server’s `/api` base URL.

## Split deploy (Vercel frontend + `hermes serve` API)

Use this when the SPA is **not** served from the same origin as the Hermes HTTP API (see `deploy/hermes-vercel-frontend-backend-separation-handoff.md`).

### Frontend (e.g. Vercel project settings)

| Variable | Example | Purpose |
|----------|---------|---------|
| `VITE_API_BASE_URL` | `https://api.example.com` | API + relative paths; no trailing `/`. Empty = same-origin (dev default). |

Protected admin calls need a **Bearer** token:

1. Same-origin classic mode: token may be injected as `window.__HERMES_SESSION_TOKEN__` by the server host.
2. Split mode: set `localStorage.hermes_api_token` to the same token value the backend expects (see backend security docs).

Optional: any future **WebSocket** client code should use `resolveWebSocketUrl("/v1/ws/...")` from `src/lib/api.ts` so `wss://` matches `https://` API bases.

### Backend (`hermes serve`)

| Variable | Example | Purpose |
|----------|---------|---------|
| `HERMES_HTTP_CORS_ORIGINS` | `https://your-app.vercel.app` | Comma-separated allowed browser `Origin` values. Empty = permissive CORS (dev-friendly). |
| `HERMES_SERVE_WEB_STATIC` | `0` | Disable serving bundled `apps/dashboard/dist` from the API process when the UI is only on Vercel. |

## Build

```bash
npm run build
```

Output directory is **`apps/dashboard/dist`** (Vite `outDir`). For Rust **same-origin** installs, the `hermes serve` may still serve that folder when `HERMES_SERVE_WEB_STATIC` is not disabled and the path exists.

## Structure

```
src/
├── components/ui/   # Reusable UI primitives (Card, Badge, Button, Input, etc.)
├── lib/
│   ├── api.ts       # API client — typed fetch wrappers for all backend endpoints
│   └── utils.ts     # cn() helper for Tailwind class merging
├── pages/
│   ├── StatusPage   # Agent status, active/recent sessions
│   ├── ConfigPage   # Dynamic config editor (reads schema from backend)
│   └── EnvPage      # API key management with save/clear
├── App.tsx          # Main layout and navigation
├── main.tsx         # React entry point
└── index.css        # Tailwind imports and theme variables
```
