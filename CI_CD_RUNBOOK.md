# Hermes Multi-Client CI/CD Runbook

本手册用于协议先行架构下的多端发布流水线操作（Electron 桌面 + Flutter 移动 + Protocol Gate）。

## Workflow 索引

- `.github/workflows/ci.yml`  
  主 CI（Rust + 可选 protocol/electron/flutter 检查）
- `.github/workflows/protocol-gate.yml`  
  协议兼容门禁（Rust ↔ TS ↔ Flutter fixture）
- `.github/workflows/desktop-release-template.yml`  
  Electron 三平台构建发布模板
- `.github/workflows/mobile-build-template.yml`  
  Flutter Android/iOS 构建模板

## 一、协议门禁（必须）

推荐将 `Protocol Gate` 设为受保护分支 required check。

它会在协议相关目录变更时自动触发，确保以下矩阵兼容：

1. Rust：`hermes-transport` fixture 解码
2. TS SDK：fixture 解码
3. Flutter：fixture 解码

手动触发：

1. 打开 GitHub Actions → `Protocol Gate`
2. 点击 `Run workflow`

## 二、Electron 桌面发布模板

工作流：`Desktop Release Template`（手动触发）

输入参数：

- `update_provider`: `github` 或 `s3`
- `upload_artifacts`: 是否上传 workflow artifact

依赖的仓库变量：

- GitHub 通道：
  - `HERMES_GH_OWNER`
  - `HERMES_GH_REPO`
  - `HERMES_GH_PRIVATE`（可选）
  - `HERMES_GH_RELEASE_TYPE`（可选）
- S3 通道：
  - `HERMES_S3_BUCKET`
  - `HERMES_S3_REGION`
  - `HERMES_S3_PATH`（可选）

产出：

- Linux / macOS / Windows 的 electron-builder 产物

## 三、Flutter 移动构建模板

工作流：`Mobile Build Template`（手动触发）

输入参数：

- `build_android`
- `build_ios`
- `run_tests`

依赖的仓库变量：

- `HERMES_MOBILE_API_BASE`（用于 `--dart-define=HERMES_API_BASE=...`）

产出：

- Android APK artifact
- iOS build 目录（no-codesign）

## 四、推荐发布顺序

1. 先跑 `Protocol Gate`（必须通过）
2. 再跑 `ci.yml` 的可选 `run_protocol_matrix`
3. Electron 发版前先跑 `run_electron_check`
4. Flutter 发版前先跑 `run_flutter_check`
5. 最后触发桌面/移动发布模板

## 五、故障排查

- **Protocol matrix 失败**：先检查 `engine/sdk/protocol-fixtures/ws_envelopes.json` 是否与 `hermes-transport` 同步。
- **Electron 构建失败**：优先检查 `apps/web-app` 是否先成功 `pnpm build:web` 与 `pnpm build:desktop`，再看 `apps/web-app/electron` 配置是否完整。
- **Mobile 构建失败**：先本地执行 `pnpm test` 与 `pnpm lint`，确认 TypeScript 类型与 Jest 都通过后再触发 EAS 构建。EAS 构建需配置 `EXPO_TOKEN` secret。
