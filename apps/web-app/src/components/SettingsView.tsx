import { useState, useEffect } from "react";
import { Save, Cpu, Globe } from "lucide-react";
import * as api from "../api";
import {
  getRuntimeInfo,
  openExternalUrl,
  openSettingsDirectory,
  pickDirectory,
  restartDesktopApp,
} from "../desktopBridge";
import type { AppConfig } from "../types";

export function SettingsView() {
  const [config, setConfig] = useState<AppConfig>({
    api_base: "http://127.0.0.1:3000",
    default_model: "",
    theme: "dark",
    mode: "local",
  });
  const [saved, setSaved] = useState(false);
  const [runtime, setRuntime] = useState("web");
  const [selectedDir, setSelectedDir] = useState<string | null>(null);

  useEffect(() => {
    api.getConfig().then(setConfig);
    getRuntimeInfo().then((info) => setRuntime(`${info.runtime} (${info.platform})`));
  }, []);

  const handleSave = async () => {
    await api.updateConfig(config);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  return (
    <div className="flex-1 overflow-y-auto px-8 py-6">
      <div className="max-w-2xl mx-auto">
        <h1 className="text-2xl font-semibold text-text-primary mb-6">设置</h1>

        <div className="space-y-6">
          <div className="rounded-xl border border-border-primary bg-bg-card p-4">
            <div className="text-sm font-medium text-text-primary">桌面运行时</div>
            <div className="text-xs text-text-muted mt-1">当前环境：{runtime}</div>
            {selectedDir && (
              <div className="text-xs text-text-muted mt-2 truncate">已选目录：{selectedDir}</div>
            )}
            <div className="flex flex-wrap gap-2 mt-3">
              <button
                onClick={() => openSettingsDirectory()}
                className="px-3 py-1.5 text-xs rounded-lg border border-border-secondary text-text-secondary hover:text-text-primary hover:border-text-muted transition-colors"
              >
                打开设置目录
              </button>
              <button
                onClick={async () => {
                  const dir = await pickDirectory();
                  if (dir) setSelectedDir(dir);
                }}
                className="px-3 py-1.5 text-xs rounded-lg border border-border-secondary text-text-secondary hover:text-text-primary hover:border-text-muted transition-colors"
              >
                选择目录（文件权限测试）
              </button>
              <button
                onClick={() => openExternalUrl("https://github.com/Lumio-Research/hermes-agent-rs")}
                className="px-3 py-1.5 text-xs rounded-lg border border-border-secondary text-text-secondary hover:text-text-primary hover:border-text-muted transition-colors"
              >
                打开项目主页
              </button>
              <button
                onClick={() => restartDesktopApp()}
                className="px-3 py-1.5 text-xs rounded-lg border border-border-secondary text-text-secondary hover:text-text-primary hover:border-text-muted transition-colors"
              >
                重启桌面应用
              </button>
            </div>
          </div>

          {/* Mode Switch */}
          <div>
            <label className="block text-sm font-medium text-text-primary mb-1">
              运行模式
            </label>
            <p className="text-xs text-text-muted mb-3">
              本地模式直接运行 Agent，流式输出；远程模式连接 hermes server 服务
            </p>
            <div className="grid grid-cols-2 gap-3">
              <button
                onClick={() => setConfig({ ...config, mode: "local" })}
                className={`flex items-center gap-3 p-4 rounded-xl border transition-colors ${
                  config.mode === "local"
                    ? "bg-accent/10 border-accent text-text-primary"
                    : "bg-bg-card border-border-primary text-text-secondary hover:border-border-secondary"
                }`}
              >
                <Cpu size={20} />
                <div className="text-left">
                  <div className="text-sm font-medium">本地模式</div>
                  <div className="text-xs text-text-muted">Agent 内嵌运行，流式输出</div>
                </div>
              </button>
              <button
                onClick={() => setConfig({ ...config, mode: "remote" })}
                className={`flex items-center gap-3 p-4 rounded-xl border transition-colors ${
                  config.mode === "remote"
                    ? "bg-accent/10 border-accent text-text-primary"
                    : "bg-bg-card border-border-primary text-text-secondary hover:border-border-secondary"
                }`}
              >
                <Globe size={20} />
                <div className="text-left">
                  <div className="text-sm font-medium">远程模式</div>
                  <div className="text-xs text-text-muted">连接 hermes server 服务</div>
                </div>
              </button>
            </div>
          </div>

          {/* Remote API Base (only shown in remote mode) */}
          {config.mode === "remote" && (
            <SettingField
              label="Hermes API 地址"
              description="hermes server 服务的地址"
            >
              <input
                type="text"
                value={config.api_base}
                onChange={(e) => setConfig({ ...config, api_base: e.target.value })}
                className="w-full bg-bg-tertiary border border-border-primary rounded-lg px-3 py-2 text-sm text-text-primary outline-none focus:border-accent transition-colors"
              />
            </SettingField>
          )}

          {/* Default Model */}
          <SettingField
            label="默认模型"
            description={config.mode === "local" ? "留空则使用 ~/.hermes/config.yaml 中的配置" : "留空则使用服务端配置"}
          >
            <input
              type="text"
              value={config.default_model}
              onChange={(e) => setConfig({ ...config, default_model: e.target.value })}
              placeholder="留空使用默认配置 (如 openrouter:z-ai/glm-5.1)"
              className="w-full bg-bg-tertiary border border-border-primary rounded-lg px-3 py-2 text-sm text-text-primary placeholder-text-muted outline-none focus:border-accent transition-colors"
            />
          </SettingField>

          {/* Theme */}
          <SettingField label="主题" description="界面外观主题">
            <select
              value={config.theme}
              onChange={(e) => setConfig({ ...config, theme: e.target.value })}
              className="w-full bg-bg-tertiary border border-border-primary rounded-lg px-3 py-2 text-sm text-text-primary outline-none focus:border-accent transition-colors"
            >
              <option value="dark">深色</option>
              <option value="light">浅色</option>
            </select>
          </SettingField>

          {/* Save */}
          <button
            onClick={handleSave}
            className="flex items-center gap-2 px-4 py-2 rounded-lg bg-accent hover:bg-accent-hover text-white text-sm transition-colors"
          >
            <Save size={16} />
            {saved ? "已保存" : "保存设置"}
          </button>
        </div>
      </div>
    </div>
  );
}

function SettingField({
  label,
  description,
  children,
}: {
  label: string;
  description: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <label className="block text-sm font-medium text-text-primary mb-1">{label}</label>
      <p className="text-xs text-text-muted mb-2">{description}</p>
      {children}
    </div>
  );
}
