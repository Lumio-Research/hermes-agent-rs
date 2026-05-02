import { useEffect, useMemo, useState } from "react";
import { Search, Plus, Sparkles, Globe, Terminal, FileCode, Database, Cpu } from "lucide-react";
import * as api from "../api";
import type { PluginSettings } from "../types";

const PLUGINS: Array<{
  id: string;
  key: keyof PluginSettings;
  name: string;
  description: string;
  icon: typeof Globe;
  category: "MCP" | "Tools";
}> = [
  { id: "mcp-filesystem", key: "mcp_filesystem", name: "Filesystem", description: "Read and write local files and folders", icon: FileCode, category: "MCP" },
  { id: "mcp-terminal", key: "mcp_terminal", name: "Terminal", description: "Execute shell commands", icon: Terminal, category: "MCP" },
  { id: "mcp-browser", key: "mcp_browser", name: "Browser", description: "Browse and search web pages", icon: Globe, category: "MCP" },
  { id: "mcp-database", key: "mcp_database", name: "Database", description: "Run SQLite and PostgreSQL queries", icon: Database, category: "MCP" },
  { id: "tool-code-exec", key: "tool_code_exec", name: "Code Exec", description: "Run code inside sandbox", icon: Cpu, category: "Tools" },
];

const DEFAULT_PLUGIN_SETTINGS: PluginSettings = {
  mcp_filesystem: true,
  mcp_terminal: true,
  mcp_browser: false,
  mcp_database: false,
  tool_code_exec: true,
};

export function PluginsView() {
  const [settings, setSettings] = useState<PluginSettings>(DEFAULT_PLUGIN_SETTINGS);
  const [loading, setLoading] = useState(true);
  const [savingId, setSavingId] = useState<string | null>(null);
  const [query, setQuery] = useState("");

  useEffect(() => {
    api.getPluginSettings()
      .then(setSettings)
      .finally(() => setLoading(false));
  }, []);

  const togglePlugin = async (
    plugin: (typeof PLUGINS)[number],
    enabled: boolean,
  ) => {
    setSavingId(plugin.id);
    try {
      const next = { ...settings, [plugin.key]: enabled };
      setSettings(next);
      const saved = await api.updatePluginSettings(next);
      setSettings(saved);
    } finally {
      setSavingId(null);
    }
  };

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return PLUGINS;
    return PLUGINS.filter((p) => `${p.name} ${p.description} ${p.category}`.toLowerCase().includes(q));
  }, [query]);

  return (
    <div className="flex-1 overflow-y-auto px-8 py-6">
      <div className="max-w-5xl mx-auto">
        <div className="flex items-center gap-4 mb-4 text-sm">
          <button className="text-text-primary font-medium">Plugins</button>
          <button className="text-text-muted hover:text-text-secondary transition-colors">Skills</button>
        </div>

        <div className="text-center mb-6">
          <h1 className="text-4xl font-semibold text-text-primary">Make Codex work your way</h1>
        </div>

        <div className="flex items-center gap-2 mb-5">
          <div className="relative flex-1">
            <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-text-muted" />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search plugins"
              className="w-full h-9 bg-bg-card border border-border-primary rounded-lg pl-9 pr-3 text-sm text-text-primary placeholder-text-muted outline-none focus:border-border-secondary"
            />
          </div>
          <button className="h-9 px-3 rounded-lg bg-bg-card border border-border-primary text-xs text-text-secondary">Codex official</button>
          <button className="h-9 px-3 rounded-lg bg-bg-card border border-border-primary text-xs text-text-secondary">All</button>
        </div>

        <div className="h-48 rounded-2xl border border-border-primary bg-[radial-gradient(120%_120%_at_15%_20%,#2d4fb5_0%,#2a3067_40%,#1a1f34_70%,#151925_100%)] mb-7 flex items-center justify-center">
          <button className="px-4 py-2 rounded-xl bg-white/85 text-[#10121a] text-sm font-medium hover:bg-white transition-colors">
            Try in chat
          </button>
        </div>

        <div className="mb-3">
          <h2 className="text-base font-medium text-text-primary">Coding</h2>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-3 pb-8">
          {filtered.map((plugin) => {
            const enabled = Boolean(settings[plugin.key]);
            return (
              <div
                key={plugin.id}
                className="group flex items-center gap-3 px-3 py-2.5 rounded-xl border border-border-primary bg-bg-card/80 hover:bg-bg-card hover:border-border-secondary transition-colors"
              >
                <div className="w-9 h-9 rounded-lg bg-bg-tertiary flex items-center justify-center text-text-secondary">
                  <plugin.icon size={16} />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="text-sm text-text-primary truncate">{plugin.name}</div>
                  <div className="text-xs text-text-muted truncate">{plugin.description}</div>
                </div>
                <button
                  onClick={() => togglePlugin(plugin, !enabled)}
                  disabled={savingId === plugin.id || loading}
                  className="w-6 h-6 rounded-full border border-border-secondary text-text-muted hover:text-text-primary hover:border-text-muted transition-colors disabled:opacity-50"
                  title={enabled ? "Disable" : "Enable"}
                >
                  {enabled ? <Sparkles size={12} className="mx-auto text-success" /> : <Plus size={12} className="mx-auto" />}
                </button>
              </div>
            );
          })}
          {!filtered.length && (
            <div className="col-span-full text-center text-sm text-text-muted py-10">No plugins matched.</div>
          )}
        </div>

        <div className="text-xs text-text-muted pb-6">
          {loading ? "Loading plugin states..." : "Plugin states are persisted by backend in remote mode."}
        </div>
      </div>
    </div>
  );
}
