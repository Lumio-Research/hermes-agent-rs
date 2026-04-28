import { useState, useEffect } from "react";
import { KeyRound, Plus, Trash2 } from "lucide-react";
import { useAuth } from "@/contexts/AuthContext";

const PROVIDERS = [
  { id: "openai", label: "OpenAI", placeholder: "sk-..." },
  { id: "anthropic", label: "Anthropic", placeholder: "sk-ant-..." },
  { id: "openrouter", label: "OpenRouter", placeholder: "sk-or-..." },
  { id: "deepseek", label: "DeepSeek", placeholder: "sk-..." },
];

export default function ApiKeysPage() {
  const { jwt } = useAuth();
  const [storedProviders, setStoredProviders] = useState<string[]>([]);
  const [showAdd, setShowAdd] = useState(false);
  const [selectedProvider, setSelectedProvider] = useState(PROVIDERS[0].id);
  const [apiKey, setApiKey] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const base = (import.meta.env.VITE_API_BASE_URL ?? "").replace(/\/$/, "");

  const fetchKeys = async () => {
    if (!jwt) return;
    try {
      const res = await fetch(`${base}/api/v1/tenant/keys`, {
        headers: { Authorization: `Bearer ${jwt}` },
      });
      if (res.ok) {
        const data = await res.json();
        setStoredProviders(data.providers ?? []);
      }
    } catch {
      // Backend may not be running
    }
  };

  useEffect(() => {
    fetchKeys();
  }, [jwt]);

  const handleAdd = async () => {
    if (!jwt || !apiKey.trim()) return;
    setLoading(true);
    setError(null);
    try {
      const res = await fetch(`${base}/api/v1/tenant/keys`, {
        method: "POST",
        headers: {
          Authorization: `Bearer ${jwt}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ provider: selectedProvider, api_key: apiKey }),
      });
      if (!res.ok) {
        const data = await res.json();
        throw new Error(data.error ?? "Failed to save key");
      }
      setApiKey("");
      setShowAdd(false);
      await fetchKeys();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "Failed to save key");
    } finally {
      setLoading(false);
    }
  };

  const handleDelete = async (provider: string) => {
    if (!jwt) return;
    if (!confirm(`Delete API key for ${provider}?`)) return;
    try {
      await fetch(`${base}/api/v1/tenant/keys?provider=${provider}`, {
        method: "DELETE",
        headers: { Authorization: `Bearer ${jwt}` },
      });
      await fetchKeys();
    } catch {
      // Handle error
    }
  };

  return (
    <div className="space-y-8">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-bold uppercase tracking-wider">
          API Keys
        </h1>
        <button
          onClick={() => setShowAdd(!showAdd)}
          className="flex items-center gap-2 rounded-lg border border-current/20 px-4 py-2 text-xs hover:bg-midground/10 transition-colors"
        >
          <Plus className="h-3.5 w-3.5" />
          Add Key
        </button>
      </div>

      <p className="text-xs text-midground/50 normal-case">
        Your API keys are encrypted with AES-256-GCM and stored securely. We
        never see your plaintext keys.
      </p>

      {/* Add key form */}
      {showAdd && (
        <div className="rounded-xl border border-current/20 p-6 space-y-4">
          {error && (
            <div className="rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-400">
              {error}
            </div>
          )}
          <div>
            <label className="block text-xs text-midground/60 uppercase tracking-wider mb-1">
              Provider
            </label>
            <select
              value={selectedProvider}
              onChange={(e) => setSelectedProvider(e.target.value)}
              className="w-full rounded-lg border border-current/20 bg-black/50 px-4 py-2.5 text-sm text-midground focus:border-midground/40 focus:outline-none"
            >
              {PROVIDERS.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.label}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="block text-xs text-midground/60 uppercase tracking-wider mb-1">
              API Key
            </label>
            <input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder={
                PROVIDERS.find((p) => p.id === selectedProvider)?.placeholder
              }
              className="w-full rounded-lg border border-current/20 bg-black/50 px-4 py-2.5 text-sm text-midground placeholder:text-midground/30 focus:border-midground/40 focus:outline-none"
            />
          </div>
          <button
            onClick={handleAdd}
            disabled={loading || !apiKey.trim()}
            className="rounded-lg bg-midground/10 border border-current/20 px-6 py-2.5 text-sm font-medium hover:bg-midground/20 disabled:opacity-50 transition-colors"
          >
            {loading ? "Saving..." : "Save Key"}
          </button>
        </div>
      )}

      {/* Stored keys */}
      <div className="space-y-3">
        {storedProviders.length === 0 ? (
          <div className="rounded-xl border border-current/10 p-8 text-center">
            <KeyRound className="h-8 w-8 mx-auto mb-3 text-midground/30" />
            <p className="text-sm text-midground/40 normal-case">
              No API keys stored yet. Add your first key to get started.
            </p>
          </div>
        ) : (
          storedProviders.map((provider) => (
            <div
              key={provider}
              className="flex items-center justify-between rounded-xl border border-current/10 px-6 py-4"
            >
              <div className="flex items-center gap-3">
                <KeyRound className="h-4 w-4 text-midground/50" />
                <span className="text-sm font-medium uppercase tracking-wider">
                  {provider}
                </span>
                <span className="text-xs text-midground/30 normal-case">
                  ••••••••
                </span>
              </div>
              <button
                onClick={() => handleDelete(provider)}
                className="text-midground/30 hover:text-red-400 transition-colors"
              >
                <Trash2 className="h-4 w-4" />
              </button>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
