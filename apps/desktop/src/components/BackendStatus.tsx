import { useState, useEffect } from "react";
import { Cpu, Wifi, WifiOff } from "lucide-react";
import * as api from "../api";

export function BackendStatus() {
  const [connected, setConnected] = useState<boolean | null>(null);
  const [mode, setMode] = useState<string>("local");

  useEffect(() => {
    const check = async () => {
      try {
        const config = await api.getConfig();
        setMode(config.mode);
        const ok = await api.checkBackendHealth();
        setConnected(ok);
      } catch {
        setConnected(false);
      }
    };
    check();
    const interval = setInterval(check, 15000);
    return () => clearInterval(interval);
  }, []);

  if (connected === null) return null;

  if (mode === "local") {
    return (
      <div className="flex items-center gap-1.5 px-2 py-1 rounded text-xs" title="本地模式 — Agent 内嵌运行">
        <Cpu size={12} className="text-success" />
        <span className="text-success">本地模式</span>
      </div>
    );
  }

  return (
    <div
      className="flex items-center gap-1.5 px-2 py-1 rounded text-xs"
      title={connected ? "已连接到远程后端" : "远程后端未连接"}
    >
      {connected ? (
        <>
          <Wifi size={12} className="text-success" />
          <span className="text-success">已连接</span>
        </>
      ) : (
        <>
          <WifiOff size={12} className="text-text-muted" />
          <span className="text-text-muted">未连接</span>
        </>
      )}
    </div>
  );
}
