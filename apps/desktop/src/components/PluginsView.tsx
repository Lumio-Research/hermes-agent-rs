import { Plus, Globe, Terminal, FileCode, Database, Cpu } from "lucide-react";

const PLUGINS = [
  { id: "mcp-filesystem", name: "文件系统", description: "读写本地文件和目录", icon: FileCode, enabled: true, category: "MCP" },
  { id: "mcp-terminal", name: "终端", description: "执行 shell 命令", icon: Terminal, enabled: true, category: "MCP" },
  { id: "mcp-browser", name: "浏览器", description: "网页浏览和搜索", icon: Globe, enabled: false, category: "MCP" },
  { id: "mcp-database", name: "数据库", description: "SQLite / PostgreSQL 查询", icon: Database, enabled: false, category: "MCP" },
  { id: "tool-code-exec", name: "代码执行", description: "在沙箱中运行代码", icon: Cpu, enabled: true, category: "Tools" },
];

export function PluginsView() {
  return (
    <div className="flex-1 overflow-y-auto px-8 py-6">
      <div className="max-w-3xl mx-auto">
        <div className="flex items-center justify-between mb-6">
          <div>
            <h1 className="text-2xl font-semibold text-text-primary">插件</h1>
            <p className="text-sm text-text-muted mt-1">
              管理 MCP 服务器和工具插件
            </p>
          </div>
          <button className="flex items-center gap-2 px-4 py-2 rounded-lg bg-bg-tertiary border border-border-primary text-sm text-text-secondary hover:bg-bg-hover hover:text-text-primary transition-colors">
            <Plus size={16} />
            添加插件
          </button>
        </div>

        {/* Plugin categories */}
        {["MCP", "Tools"].map((category) => (
          <div key={category} className="mb-8">
            <h2 className="text-base font-medium text-text-primary mb-3">
              {category}
            </h2>
            <div className="space-y-2">
              {PLUGINS.filter((p) => p.category === category).map((plugin) => (
                <div
                  key={plugin.id}
                  className="flex items-center gap-4 p-4 rounded-xl bg-bg-card border border-border-primary"
                >
                  <div className="w-10 h-10 rounded-lg bg-bg-tertiary flex items-center justify-center text-text-secondary">
                    <plugin.icon size={20} />
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="text-sm font-medium text-text-primary">
                      {plugin.name}
                    </div>
                    <div className="text-xs text-text-muted">
                      {plugin.description}
                    </div>
                  </div>
                  <label className="relative inline-flex items-center cursor-pointer">
                    <input
                      type="checkbox"
                      defaultChecked={plugin.enabled}
                      className="sr-only peer"
                    />
                    <div className="w-9 h-5 bg-bg-hover rounded-full peer peer-checked:bg-accent transition-colors after:content-[''] after:absolute after:top-0.5 after:left-[2px] after:bg-white after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:after:translate-x-full" />
                  </label>
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
