# 桌面客户端历史能力对照清单

## 目标

在协议层统一后，Electron 作为桌面壳覆盖历史桌面实现能力，确保迁移平滑。

## 当前对齐状态

| 能力 | 历史桌面实现 | Electron | 状态 |
|---|---|---|---|
| 会话/消息协议调用（HTTP + WS） | 已支持 | 已支持（复用 `apps/web-app`） | ✅ |
| 设置读写 | `get_config/update_config` | 复用前端 `api.ts` | ✅ |
| 打开设置目录 | `plugin-shell + appDataDir` | `ipc: desktop:open-settings-dir` | ✅ |
| 目录选择（权限确认） | `plugin-dialog` | `ipc: desktop:pick-directory` | ✅ |
| 外链打开 | `plugin-shell open` | `ipc: desktop:open-external` | ✅ |
| 应用重启 | 可扩展 | `ipc: desktop:restart-app` + 设置页入口 | ✅ |
| 自动更新 | 历史方案可接 | `electron-updater` 已接 | ✅ |
| 崩溃上报 | 可接 Sentry | `crashReporter` + `telemetry` 统一上报 | ✅ |
| 系统托盘/全局快捷键 | 原实现未落地 | 与原能力保持一致（未做） | ✅ |

## 补齐策略

1. 保留 `apps/web-app` 为唯一 UI 代码源，桌面壳仅做系统能力适配。
2. 所有桌面特有能力通过 bridge 暴露，业务层不再绑定具体壳层 API。
3. 历史桌面实现已从 workspace 与前端依赖中移除，桌面发行统一走 Electron。
