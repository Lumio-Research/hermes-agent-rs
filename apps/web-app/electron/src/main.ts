import { app, BrowserWindow, crashReporter, dialog, ipcMain, shell } from "electron";
import log from "electron-log";
import { autoUpdater } from "electron-updater";
import { randomUUID } from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const isDev = process.env.NODE_ENV === "development";
const devUrl = process.env.HERMES_ELECTRON_DEV_URL || "http://127.0.0.1:1420";

let mainWindow: BrowserWindow | null = null;

async function emitTelemetry(level: "info" | "warn" | "error", message: string): Promise<void> {
  const base = process.env.HERMES_API_BASE;
  if (!base) return;
  const url = `${base.replace(/\/$/, "")}/v1/telemetry/client-event`;
  try {
    await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        runtime: "electron-main",
        level,
        message,
        app_version: app.getVersion(),
        trace_id: randomUUID(),
        tags: {
          platform: process.platform,
        },
      }),
    });
  } catch {
    // best effort
  }
}

function appSettingsDir(): string {
  return path.join(app.getPath("userData"), "settings");
}

function assertWithinSettingsDir(targetPath: string): string {
  const normalized = path.resolve(targetPath);
  const allowedRoot = path.resolve(appSettingsDir());
  if (!normalized.startsWith(allowedRoot)) {
    throw new Error("path access denied");
  }
  return normalized;
}

function registerIpcHandlers(): void {
  ipcMain.handle("desktop:get-runtime-info", () => ({
    runtime: "electron",
    platform: process.platform,
    userDataDir: app.getPath("userData"),
  }));

  ipcMain.handle("desktop:open-settings-dir", async () => {
    const dir = appSettingsDir();
    await fs.mkdir(dir, { recursive: true });
    await shell.openPath(dir);
    return dir;
  });

  ipcMain.handle("desktop:pick-directory", async () => {
    const result = await dialog.showOpenDialog({
      properties: ["openDirectory", "createDirectory"],
    });
    if (result.canceled || result.filePaths.length === 0) {
      return null;
    }
    return result.filePaths[0];
  });

  ipcMain.handle("desktop:read-settings-file", async (_event, relativePath: string) => {
    const safePath = assertWithinSettingsDir(path.join(appSettingsDir(), relativePath));
    return fs.readFile(safePath, "utf-8");
  });

  ipcMain.handle(
    "desktop:write-settings-file",
    async (_event, relativePath: string, content: string) => {
      const safePath = assertWithinSettingsDir(path.join(appSettingsDir(), relativePath));
      await fs.mkdir(path.dirname(safePath), { recursive: true });
      await fs.writeFile(safePath, content, "utf-8");
      return true;
    },
  );

  ipcMain.handle("desktop:open-external", async (_event, url: string) => {
    await shell.openExternal(url);
    return true;
  });

  ipcMain.handle("desktop:restart-app", () => {
    app.relaunch();
    app.exit(0);
    return true;
  });
}

function setupCrashReporting(): void {
  crashReporter.start({
    productName: "Hermes Desktop",
    companyName: "Hermes",
    submitURL: process.env.HERMES_CRASH_SUBMIT_URL || "",
    uploadToServer: Boolean(process.env.HERMES_CRASH_SUBMIT_URL),
    compress: true,
  });
}

function setupAutoUpdate(): void {
  autoUpdater.logger = log;
  autoUpdater.autoDownload = true;
  autoUpdater.autoInstallOnAppQuit = true;
  if (!app.isPackaged) {
    log.info("autoUpdater disabled in development mode");
    return;
  }

  autoUpdater.on("error", (err) => {
    log.error("autoUpdater error", err);
    void emitTelemetry("error", `autoUpdater error: ${String(err)}`);
  });

  autoUpdater.on("update-downloaded", async () => {
    void emitTelemetry("info", "update downloaded");
    const ret = await dialog.showMessageBox({
      type: "info",
      title: "Update Ready",
      message: "A new Hermes Desktop update has been downloaded.",
      detail: "Restart now to apply update?",
      buttons: ["Restart", "Later"],
      defaultId: 0,
      cancelId: 1,
    });
    if (ret.response === 0) {
      autoUpdater.quitAndInstall();
    }
  });
}

async function createMainWindow(): Promise<void> {
  mainWindow = new BrowserWindow({
    width: 1280,
    height: 860,
    minWidth: 980,
    minHeight: 680,
    show: false,
    webPreferences: {
      preload: path.join(__dirname, "preload.js"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });

  mainWindow.once("ready-to-show", () => {
    mainWindow?.show();
  });

  mainWindow.webContents.setWindowOpenHandler(({ url }) => {
    // Keep external windows disabled by default for security.
    log.warn(`blocked window.open: ${url}`);
    return { action: "deny" };
  });

  if (isDev) {
    await mainWindow.loadURL(devUrl);
    mainWindow.webContents.openDevTools({ mode: "detach" });
  } else {
    const indexPath = path.resolve(__dirname, "../../dist/index.html");
    await mainWindow.loadFile(indexPath);
  }
}

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});

app.on("activate", async () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    await createMainWindow();
  }
});

app.whenReady()
  .then(async () => {
    process.on("uncaughtException", (err) => {
      log.error("uncaughtException", err);
      void emitTelemetry("error", `uncaughtException: ${String(err)}`);
    });
    process.on("unhandledRejection", (reason) => {
      log.error("unhandledRejection", reason);
      void emitTelemetry("error", `unhandledRejection: ${String(reason)}`);
    });

    setupCrashReporting();
    setupAutoUpdate();
    registerIpcHandlers();
    await createMainWindow();
    void emitTelemetry("info", "electron main booted");
    // Trigger update check after first render.
    void autoUpdater.checkForUpdatesAndNotify();
  })
  .catch((err) => {
    log.error("Electron bootstrap failed", err);
    app.exit(1);
  });
