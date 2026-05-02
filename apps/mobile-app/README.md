# Hermes Mobile (React Native + Expo)

iOS / Android mobile client built with Expo React Native.  
Shares protocol types and API client logic with `apps/web-app`.

## Run

```bash
cd apps/mobile-app
pnpm install
pnpm start          # Expo dev server (scan QR with Expo Go)
pnpm ios            # Run on iOS simulator
pnpm android        # Run on Android emulator
```

Connect to your Hermes backend:

```bash
# Open Settings in the app and set:
# API Base URL: http://<your-machine-ip>:8787
```

For iOS simulator, the default `http://127.0.0.1:8787` works directly.  
For Android emulator, use `http://10.0.2.2:8787`.

## Build (EAS)

```bash
pnpm build:ios
pnpm build:android
```

## Test

```bash
pnpm test   # Jest (protocol fixture compat test)
```
