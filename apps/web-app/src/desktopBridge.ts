type RuntimeInfo = {
  runtime: "electron" | "web";
  platform: string;
  userDataDir?: string;
};

type HermesDesktopBridge = {
  runtime: "electron";
  platform: string;
  getRuntimeInfo: () => Promise<RuntimeInfo>;
  openSettingsDir: () => Promise<string>;
  pickDirectory: () => Promise<string | null>;
  readSettingsFile: (relativePath: string) => Promise<string>;
  writeSettingsFile: (relativePath: string, content: string) => Promise<boolean>;
  openExternal: (url: string) => Promise<boolean>;
  restartApp: () => Promise<boolean>;
};

declare global {
  interface Window {
    hermesDesktop?: HermesDesktopBridge;
  }
}

export function isElectronRuntime(): boolean {
  return typeof window !== "undefined" && Boolean(window.hermesDesktop);
}

export async function getRuntimeInfo(): Promise<RuntimeInfo> {
  if (isElectronRuntime()) {
    return window.hermesDesktop!.getRuntimeInfo();
  }
  return {
    runtime: "web",
    platform: typeof navigator !== "undefined" ? navigator.platform : "unknown",
  };
}

export async function openSettingsDirectory(): Promise<void> {
  if (isElectronRuntime()) {
    await window.hermesDesktop!.openSettingsDir();
    return;
  }
  throw new Error("当前环境不支持打开设置目录");
}

export async function pickDirectory(): Promise<string | null> {
  if (isElectronRuntime()) {
    return window.hermesDesktop!.pickDirectory();
  }
  return null;
}

export async function openExternalUrl(url: string): Promise<void> {
  if (isElectronRuntime()) {
    await window.hermesDesktop!.openExternal(url);
    return;
  }
  window.open(url, "_blank", "noopener,noreferrer");
}

export async function restartDesktopApp(): Promise<void> {
  if (isElectronRuntime()) {
    await window.hermesDesktop!.restartApp();
    return;
  }
  throw new Error("当前环境不支持重启应用");
}
