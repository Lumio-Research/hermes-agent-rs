import { useState, useEffect } from "react";
import { Globe, CheckCircle, XCircle, AlertCircle } from "lucide-react";
import { useAuth } from "@/contexts/AuthContext";

const PLATFORMS = [
  { id: "telegram", label: "Telegram", fields: [{ key: "token", label: "Bot Token", placeholder: "123456:ABC-DEF..." }] },
  { id: "discord", label: "Discord", fields: [{ key: "token", label: "Bot Token", placeholder: "MTIz..." }] },
  { id: "slack", label: "Slack", fields: [{ key: "token", label: "Bot Token (xoxb-...)", placeholder: "xoxb-..." }] },
  { id: "weixin", label: "WeChat", fields: [{ key: "token", label: "Token", placeholder: "your-token" }, { key: "account_id", label: "Account ID", placeholder: "wxid_..." }] },
  { id: "whatsapp", label: "WhatsApp", fields: [{ key: "token", label: "Cloud API Token", placeholder: "EAAx..." }, { key: "phone_number_id", label: "Phone Number ID", placeholder: "123..." }] },
  { id: "signal", label: "Signal", fields: [{ key: "phone_number", label: "Phone Number", placeholder: "+1555..." }] },
  { id: "matrix", label: "Matrix", fields: [{ key: "token", label: "Access Token", placeholder: "syt_..." }, { key: "homeserver_url", label: "Homeserver URL", placeholder: "https://matrix.org" }] },
  { id: "dingtalk", label: "DingTalk", fields: [{ key: "client_id", label: "Client ID", placeholder: "..." }, { key: "client_secret", label: "Client Secret", placeholder: "..." }] },
  { id: "feishu", label: "Feishu/Lark", fields: [{ key: "app_id", label: "App ID", placeholder: "cli_..." }, { key: "app_secret", label: "App Secret", placeholder: "..." }] },
];

type ConnectionStatus = "connected" | "disconnected" | "error";

interface PlatformStatus {
  platform: string;
  status: ConnectionStatus;
  error?: string;
}

export default function PlatformConnectionsPage() {
  const { jwt, plan } = useAuth();
  const [statuses, setStatuses] = useState<PlatformStatus[]>([]);
  const [setupPlatform, setSetupPlatform] = useState<string | null>(null);
  const [fieldValues, setFieldValues] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const base = (import.meta.env.VITE_API_BASE_URL ?? "").replace(/\/$/, "");

  const fetchStatuses = async () => {
    if (!jwt) return;
    try {
      const res = await fetch(`${base}/api/v1/tenant/platforms`, {
        headers: { Authorization: `Bearer ${jwt}` },
      });
      if (res.ok) {
        const data = await res.json();
        setStatuses(data.platforms ?? []);
      }
    } catch { /* backend may not be running */ }
  };

  useEffect(() => { fetchStatuses(); }, [jwt]);

  const handleConnect = async (platformId: string) => {
    if (!jwt) return;
    setSaving(true);
    setError(null);
    try {
      const res = await fetch(`${base}/api/v1/tenant/platforms`, {
        method: "POST",
        headers: { Authorization: `Bearer ${jwt}`, "Content-Type": "application/json" },
        body: JSON.stringify({ platform: platformId, credentials: fieldValues }),
      });
      if (!res.ok) {
        const data = await res.json();
        throw new Error(data.error ?? "Failed to connect");
      }
      setSetupPlatform(null);
      setFieldValues({});
      await fetchStatuses();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "Connection failed");
    } finally {
      setSaving(false);
    }
  };

  const statusIcon = (s: ConnectionStatus) => {
    switch (s) {
      case "connected": return <CheckCircle className="h-4 w-4 text-green-400" />;
      case "error": return <XCircle className="h-4 w-4 text-red-400" />;
      default: return <AlertCircle className="h-4 w-4 text-midground/30" />;
    }
  };

  if (plan !== "cloud_pro") {
    return (
      <div className="space-y-8">
        <h1 className="text-xl font-bold uppercase tracking-wider">Platform Connections</h1>
        <div className="rounded-xl border border-current/10 p-8 text-center">
          <Globe className="h-10 w-10 mx-auto mb-4 text-midground/30" />
          <p className="text-sm text-midground/50 normal-case mb-4">
            Platform connections require Cloud Pro. Connect Telegram, Discord, Slack, WeChat, and 13 more platforms.
          </p>
          <a href="/dashboard/subscription" className="inline-block rounded-lg bg-midground/10 border border-current/20 px-6 py-2.5 text-sm hover:bg-midground/20 transition-colors">
            Upgrade to Cloud Pro
          </a>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-8">
      <h1 className="text-xl font-bold uppercase tracking-wider">Platform Connections</h1>

      {/* Connected platforms */}
      {statuses.length > 0 && (
        <div className="space-y-3">
          <h2 className="text-xs text-midground/50 uppercase tracking-wider">Active Connections</h2>
          {statuses.map((s) => (
            <div key={s.platform} className="flex items-center justify-between rounded-xl border border-current/10 px-6 py-4">
              <div className="flex items-center gap-3">
                {statusIcon(s.status)}
                <span className="text-sm font-medium uppercase tracking-wider">{s.platform}</span>
              </div>
              <span className="text-xs text-midground/40 normal-case">{s.status}{s.error ? `: ${s.error}` : ""}</span>
            </div>
          ))}
        </div>
      )}

      {/* Available platforms */}
      <div className="space-y-3">
        <h2 className="text-xs text-midground/50 uppercase tracking-wider">Available Platforms</h2>
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
          {PLATFORMS.map((p) => {
            const connected = statuses.some((s) => s.platform === p.id && s.status === "connected");
            return (
              <button
                key={p.id}
                onClick={() => { setSetupPlatform(p.id); setFieldValues({}); setError(null); }}
                disabled={connected}
                className={`rounded-xl border px-5 py-4 text-left transition-colors ${
                  connected
                    ? "border-green-500/20 bg-green-500/5 opacity-60"
                    : "border-current/10 hover:border-current/20 hover:bg-midground/5"
                }`}
              >
                <div className="flex items-center gap-2 mb-1">
                  <Globe className="h-4 w-4" />
                  <span className="text-sm font-medium uppercase tracking-wider">{p.label}</span>
                </div>
                <span className="text-xs text-midground/40 normal-case">
                  {connected ? "Connected" : "Click to set up"}
                </span>
              </button>
            );
          })}
        </div>
      </div>

      {/* Setup modal */}
      {setupPlatform && (() => {
        const platform = PLATFORMS.find((p) => p.id === setupPlatform);
        if (!platform) return null;
        return (
          <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 px-4">
            <div className="w-full max-w-md rounded-xl border border-current/20 bg-black p-6 space-y-4">
              <h3 className="text-lg font-bold uppercase tracking-wider">
                Connect {platform.label}
              </h3>
              {error && (
                <div className="rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-400">{error}</div>
              )}
              {platform.fields.map((f) => (
                <div key={f.key}>
                  <label className="block text-xs text-midground/60 uppercase tracking-wider mb-1">{f.label}</label>
                  <input
                    type="password"
                    value={fieldValues[f.key] ?? ""}
                    onChange={(e) => setFieldValues({ ...fieldValues, [f.key]: e.target.value })}
                    placeholder={f.placeholder}
                    className="w-full rounded-lg border border-current/20 bg-black/50 px-4 py-2.5 text-sm text-midground placeholder:text-midground/30 focus:border-midground/40 focus:outline-none"
                  />
                </div>
              ))}
              <div className="flex gap-3 pt-2">
                <button
                  onClick={() => handleConnect(setupPlatform)}
                  disabled={saving}
                  className="flex-1 rounded-lg bg-midground/10 border border-current/20 py-2.5 text-sm font-medium hover:bg-midground/20 disabled:opacity-50 transition-colors"
                >
                  {saving ? "Connecting..." : "Connect"}
                </button>
                <button
                  onClick={() => setSetupPlatform(null)}
                  className="rounded-lg border border-current/20 px-6 py-2.5 text-sm hover:bg-midground/10 transition-colors"
                >
                  Cancel
                </button>
              </div>
            </div>
          </div>
        );
      })()}
    </div>
  );
}
