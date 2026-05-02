import { apiUrl } from "./api";
import { getRuntimeInfo } from "./desktopBridge";

type TelemetryLevel = "info" | "warn" | "error";

export async function sendClientTelemetry(
  level: TelemetryLevel,
  message: string,
  tags?: Record<string, unknown>,
): Promise<void> {
  try {
    const runtime = await getRuntimeInfo();
    await fetch(apiUrl("/v1/telemetry/client-event"), {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        runtime: runtime.runtime,
        level,
        message,
        app_version: (import.meta as ImportMeta).env?.VITE_APP_VERSION || "dev",
        trace_id: crypto?.randomUUID?.() || undefined,
        tags: {
          platform: runtime.platform,
          ...(tags || {}),
        },
      }),
    });
  } catch {
    // swallow telemetry failures
  }
}

export function installGlobalClientTelemetryHooks(): void {
  window.addEventListener("error", (event) => {
    void sendClientTelemetry("error", event.message || "window error", {
      filename: event.filename,
      lineno: event.lineno,
      colno: event.colno,
    });
  });

  window.addEventListener("unhandledrejection", (event) => {
    void sendClientTelemetry(
      "error",
      `unhandled rejection: ${String(event.reason)}`,
    );
  });
}
