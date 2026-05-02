# Hermes Electron Client

Electron desktop shell merged into `apps/web-app`.

## Development

```bash
cd apps/web-app
pnpm install
pnpm dev:web      # start renderer dev server
pnpm dev:desktop  # start electron shell
```

By default the app loads `http://127.0.0.1:1420` in dev mode. Override with:

```bash
HERMES_ELECTRON_DEV_URL=http://127.0.0.1:3000 pnpm dev
```

## Packaging

```bash
pnpm dist       # mac + win + linux
pnpm dist:mac
pnpm dist:win
pnpm dist:linux
```

### Auto-update channels

GitHub Releases:

```bash
export HERMES_GH_OWNER=your-org
export HERMES_GH_REPO=hermes-desktop
pnpm dist:github
```

S3:

```bash
export HERMES_S3_BUCKET=your-bucket
export HERMES_S3_REGION=us-east-1
export HERMES_S3_PATH=desktop-updates
pnpm dist:s3
```

## Notes

- Auto-update uses `electron-updater` with GitHub Releases or S3 publish config.
- Crash reporting upload is disabled unless `HERMES_CRASH_SUBMIT_URL` is configured.
