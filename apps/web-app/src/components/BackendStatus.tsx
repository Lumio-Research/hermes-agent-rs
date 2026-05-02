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
      <div className="flex items-center gap-1.5 px-2 py-1 rounded text-xs" title="Local mode — embedded agent runtime">
        <Cpu size={12} className="text-success" />
        <span className="text-[#22c55e]">Connected</span>
      </div>
    );
  }

  return (
    <div
      className="flex items-center gap-1.5 px-2 py-1 rounded text-xs"
      title={connected ? "Connected to remote backend" : "Remote backend disconnected"}
    >
      {connected ? (
        <>
          <Wifi size={12} className="text-success" />
          <span className="text-[#22c55e]">Connected</span>
        </>
      ) : (
        <>
          <WifiOff size={12} className="text-[#7d879d]" />
          <span className="text-[#7d879d]">Offline</span>
        </>
      )}
    </div>
  );
}
