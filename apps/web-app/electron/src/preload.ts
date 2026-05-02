import { contextBridge } from "electron";
import { ipcRenderer } from "electron";

contextBridge.exposeInMainWorld("hermesDesktop", {
  platform: process.platform,
  runtime: "electron",
  getRuntimeInfo: () => ipcRenderer.invoke("desktop:get-runtime-info"),
  openSettingsDir: () => ipcRenderer.invoke("desktop:open-settings-dir"),
  pickDirectory: () => ipcRenderer.invoke("desktop:pick-directory"),
  readSettingsFile: (relativePath: string) =>
    ipcRenderer.invoke("desktop:read-settings-file", relativePath),
  writeSettingsFile: (relativePath: string, content: string) =>
    ipcRenderer.invoke("desktop:write-settings-file", relativePath, content),
  openExternal: (url: string) => ipcRenderer.invoke("desktop:open-external", url),
  restartApp: () => ipcRenderer.invoke("desktop:restart-app"),
});
