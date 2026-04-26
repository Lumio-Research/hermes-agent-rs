<div align="center">

# ⚡ Hermes Agent

**自我进化的 AI Agent。一个二进制文件，全平台运行。**

[Nous Research](https://nousresearch.com) 出品的 [Hermes Agent](https://github.com/NousResearch/hermes-agent) Rust 重写版。

`110,000+ 行 Rust 代码` · `1,428 个测试` · `17 个 crate` · `~16MB 二进制`

**[English](./README.md)** · **[中文](./README_ZH.md)** · **[日本語](./README_JA.md)** · **[한국어](./README_KO.md)**

</div>

---

## 为什么选择 Hermes？

🚀 **零依赖** — 单个静态二进制文件。不需要 Python、pip、Docker。复制到树莓派、$3/月 VPS 或断网服务器，直接运行。

🧠 **自进化引擎** — 多臂老虎机模型选择、长任务规划、Prompt 与记忆自动塑形。用得越多，Agent 越聪明。

🔌 **17 个平台 · 30+ 工具 · 8 个记忆后端** — Telegram、Discord、Slack、微信、钉钉、飞书、企业微信等 17 个平台。文件操作、浏览器、代码执行、视觉、语音、网页搜索、Home Assistant 等 30+ 工具。

⚡ **真并发** — Rust tokio 运行时将工具调用分发到 OS 线程。30 秒的浏览器抓取不会阻塞 50ms 的文件读取。没有 GIL。

## 快速开始

```bash
# 安装
curl -fsSL https://raw.githubusercontent.com/Lumio-Research/hermes-agent-rs/main/scripts/install.sh | bash

# 设置 API 密钥
echo "ANTHROPIC_API_KEY=sk-..." >> ~/.hermes/.env

# 运行
hermes
```

就这样。你已经进入了一个带工具、记忆和流式输出的交互式会话。

## 能做什么？

**与任意 LLM 对话** — 对话中随时切换模型：
```
hermes
> /model gpt-4o
> 分析这个仓库，找出安全问题
```

**命令行一次性任务**：
```bash
hermes chat --query "把 auth.rs 重构为新的错误类型"
```

**多平台网关** — 同时连接 Telegram、Discord、Slack、微信等：
```bash
hermes gateway start
```

**随处运行** — Docker、SSH 或远程沙箱：
```yaml
# ~/.hermes/config.yaml
terminal:
  backend: docker
  image: ubuntu:24.04
```

**MCP + ACP** — 连接外部工具服务器，或将 Hermes 暴露为工具服务：
```yaml
mcp:
  servers:
    - name: my-tools
      command: npx my-mcp-server
```

**语音模式** — VAD + STT + TTS 流水线，解放双手。

## 架构

```
hermes-cli                    # 二进制入口、TUI、斜杠命令
├── hermes-agent              # Agent 循环、LLM 提供商、记忆插件
│   ├── hermes-core           # 共享类型、trait、错误层级
│   ├── hermes-intelligence   # 模型路由、Prompt 构建、自进化
│   └── hermes-config         # 配置加载、YAML/环境变量合并
├── hermes-tools              # 30+ 工具后端、审批引擎
├── hermes-gateway            # 17 个平台适配器、会话管理
├── hermes-environments       # 终端：Local/Docker/SSH/Daytona/Modal/Singularity
├── hermes-mcp                # Model Context Protocol 客户端/服务端
├── hermes-acp                # Agent Communication Protocol
├── hermes-skills             # 技能管理与 Hub
├── hermes-cron               # 定时任务调度
├── hermes-dashboard          # Web 控制台 + HTTP/WebSocket API 服务
├── hermes-auth               # OAuth 令牌交换
├── hermes-eval               # SWE-bench、Terminal-Bench、YC Bench
└── hermes-telemetry          # OpenTelemetry + Prometheus
```

**核心 trait：** `LlmProvider`（10 个提供商）· `ToolHandler`（30+ 后端）· `PlatformAdapter`（17 个平台）· `TerminalBackend`（6 个后端）· `MemoryProvider`（8 个插件）

**工具调用解析器：** Hermes、Anthropic、OpenAI、Qwen、Llama、DeepSeek、Auto

## 安装

**一键安装**（自动识别系统与架构）：
```bash
curl -fsSL https://raw.githubusercontent.com/Lumio-Research/hermes-agent-rs/main/scripts/install.sh | bash
```

**从源码安装：**
```bash
cargo install --git https://github.com/Lumio-Research/hermes-agent-rs hermes-cli --locked
```

**手动下载：** 从 [Releases](https://github.com/Lumio-Research/hermes-agent-rs/releases) 下载对应平台的二进制文件。

**Docker：**
```bash
docker run --rm -it -v ~/.hermes:/root/.hermes ghcr.io/lumio-research/hermes-agent-rs
```

## 贡献

欢迎贡献。提交前请运行测试：

```bash
cargo test --workspace        # 1,428 个测试
cargo clippy --workspace      # 代码检查
cargo fmt --all --check       # 格式检查
```

架构细节和编码规范见 [AGENTS.md](AGENTS.md)。

## 许可证

MIT — 见 [LICENSE](LICENSE)。

基于 [Nous Research](https://nousresearch.com) 的 [Hermes Agent](https://github.com/NousResearch/hermes-agent)。
