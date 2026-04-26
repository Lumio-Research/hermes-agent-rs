<div align="center">

# ⚡ Hermes Agent `v0.1`

**The self-evolving AI agent. One binary. Every platform.**

Rust rewrite of [Hermes Agent](https://github.com/NousResearch/hermes-agent) by [Nous Research](https://nousresearch.com).

`110,000+ lines of Rust` · `1,428 tests` · `17 crates` · `~16MB binary`

**[English](./README.md)** · **[中文](./README_ZH.md)** · **[日本語](./README_JA.md)** · **[한국어](./README_KO.md)**

</div>

---

> **v0.1 Status:** Core agent loop, 10 LLM providers, 30 tool backends, 17 platform adapters, memory system, and CLI/TUI are production-ready. Known limitations: parity roadmap modules outside this runtime surface are still being completed (see `PARITY_PLAN.md`).

## Why Hermes?

🚀 **Zero dependencies** — Single static binary. No Python, no pip, no Docker. Copy it to a Raspberry Pi, a $3 VPS, or an air-gapped server and run it.

🧠 **Self-evolution engine** — Multi-armed bandit model selection, long-task planning, and prompt/memory shaping. The agent gets better the more you use it.

🔌 **17 platforms, 30+ tools, 8 memory backends** — Telegram, Discord, Slack, WhatsApp, Signal, Matrix, and 11 more. File ops, browser, code execution, vision, voice, web search, Home Assistant, and beyond.

⚡ **True concurrency** — Rust's tokio runtime dispatches tool calls across OS threads. A 30-second browser scrape doesn't block a 50ms file read. No GIL.

## Quick Start

```bash
# Install
curl -fsSL https://raw.githubusercontent.com/Lumio-Research/hermes-agent-rs/main/scripts/install.sh | bash

# Set your API key
echo "ANTHROPIC_API_KEY=sk-..." >> ~/.hermes/.env

# Run
hermes
```

That's it. You're in an interactive session with tool access, memory, and streaming.

## What Can It Do?

**Chat with any LLM** — switch models mid-conversation:
```
hermes
> /model gpt-4o
> Summarize this repo and find security issues
```

**One-shot tasks** from the command line:
```bash
hermes chat --query "Refactor auth.rs to use the new error types"
```

**Multi-platform gateway** — connect Telegram, Discord, Slack, and more simultaneously:
```bash
hermes gateway start
```

**Run anywhere** — Docker, SSH, or remote sandboxes:
```yaml
# ~/.hermes/config.yaml
terminal:
  backend: docker
  image: ubuntu:24.04
```

**MCP + ACP** — connect external tool servers or expose Hermes as one:
```yaml
mcp:
  servers:
    - name: my-tools
      command: npx my-mcp-server
```

**Voice mode** — VAD + STT + TTS pipeline for hands-free interaction.

## Architecture

```
hermes-cli                    # Binary entry point, TUI, slash commands
├── hermes-agent              # Agent loop, LLM providers, memory plugins
│   ├── hermes-core           # Shared types, traits, error hierarchy
│   ├── hermes-intelligence   # Model routing, prompt building, self-evolution
│   └── hermes-config         # Config loading, YAML/env merging
├── hermes-tools              # 30+ tool backends, approval engine
├── hermes-gateway            # 17 platform adapters, session management
├── hermes-environments       # Terminal: Local/Docker/SSH/Daytona/Modal/Singularity
├── hermes-mcp                # Model Context Protocol client/server
├── hermes-acp                # Agent Communication Protocol
├── hermes-skills             # Skill management and hub
├── hermes-cron               # Cron scheduling
├── hermes-dashboard          # Web dashboard + HTTP/WebSocket API server
├── hermes-auth               # OAuth token exchange
├── hermes-eval               # SWE-bench, Terminal-Bench, YC Bench
└── hermes-telemetry          # OpenTelemetry + Prometheus
```

**Key traits:** `LlmProvider` (10 providers) · `ToolHandler` (30+ backends) · `PlatformAdapter` (17 platforms) · `TerminalBackend` (6 backends) · `MemoryProvider` (8 plugins)

**Tool call parsers:** Hermes, Anthropic, OpenAI, Qwen, Llama, DeepSeek, Auto

## Install

**One-liner** (auto-detects OS and CPU):
```bash
curl -fsSL https://raw.githubusercontent.com/Lumio-Research/hermes-agent-rs/main/scripts/install.sh | bash
```

**From source:**
```bash
cargo install --git https://github.com/Lumio-Research/hermes-agent-rs hermes-cli --locked
```

**Manual download:** grab the binary for your platform from [Releases](https://github.com/Lumio-Research/hermes-agent-rs/releases).

**Docker:**
```bash
docker run --rm -it -v ~/.hermes:/root/.hermes ghcr.io/lumio-research/hermes-agent-rs
```

## Contributing

Contributions welcome. Run the test suite before submitting:

```bash
cargo test --workspace        # 1,428 tests
cargo clippy --workspace --all-targets -- -D warnings  # Lint (warnings fail CI)
cargo fmt --all --check       # Format
bash scripts/ci/smoke.sh      # Release binary smoke check
bash scripts/ci/keypath-e2e.sh  # Core end-to-end paths
```

See [AGENTS.md](AGENTS.md) for architecture details and coding conventions.
Plugin hook payload contracts are documented in [HOOK_PAYLOAD_SCHEMA.md](HOOK_PAYLOAD_SCHEMA.md).

## License

MIT — see [LICENSE](LICENSE).

Based on [Hermes Agent](https://github.com/NousResearch/hermes-agent) by [Nous Research](https://nousresearch.com).
