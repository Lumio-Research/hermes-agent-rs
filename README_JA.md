<div align="center">

# ⚡ Hermes Agent

**自己進化する AI エージェント。バイナリひとつで、あらゆるプラットフォームに対応。**

[Nous Research](https://nousresearch.com) の [Hermes Agent](https://github.com/NousResearch/hermes-agent) を Rust で書き直したプロダクション版。

`110,000行超の Rust` · `1,428 テスト` · `17 クレート` · `~16MB バイナリ`

**[English](./README.md)** · **[中文](./README_ZH.md)** · **[日本語](./README_JA.md)** · **[한국어](./README_KO.md)**

</div>

---

## なぜ Hermes？

🚀 **依存関係ゼロ** — 単一の静的バイナリ。Python も pip も Docker も不要。Raspberry Pi、月額 $3 の VPS、エアギャップサーバーにコピーして、そのまま実行。

🧠 **自己進化エンジン** — マルチアームドバンディットによるモデル選択、長期タスク計画、プロンプトとメモリの自動最適化。使うほどエージェントが賢くなる。

🔌 **17 プラットフォーム · 30 以上のツール · 8 つのメモリバックエンド** — Telegram、Discord、Slack、WhatsApp、Signal、Matrix ほか 11 プラットフォーム。ファイル操作、ブラウザ、コード実行、画像認識、音声、Web 検索、Home Assistant など。

⚡ **真の並行処理** — Rust の tokio ランタイムがツール呼び出しを OS スレッドに分散。30 秒のブラウザスクレイプが 50ms のファイル読み取りをブロックしない。GIL なし。

## クイックスタート

```bash
# インストール
curl -fsSL https://raw.githubusercontent.com/Lumio-Research/hermes-agent-rs/main/scripts/install.sh | bash

# API キーを設定
echo "ANTHROPIC_API_KEY=sk-..." >> ~/.hermes/.env

# 起動
hermes
```

これだけ。ツール、メモリ、ストリーミング出力が使えるインタラクティブセッションが始まる。

## 何ができる？

**あらゆる LLM と対話** — 会話中にモデルを切り替え：
```
hermes
> /model gpt-4o
> このリポジトリを分析して、セキュリティの問題を見つけて
```

**コマンドラインからワンショット実行**：
```bash
hermes chat --query "auth.rs を新しいエラー型にリファクタリングして"
```

**マルチプラットフォームゲートウェイ** — Telegram、Discord、Slack などを同時接続：
```bash
hermes gateway start
```

**どこでも実行** — Docker、SSH、リモートサンドボックス：
```yaml
# ~/.hermes/config.yaml
terminal:
  backend: docker
  image: ubuntu:24.04
```

**MCP + ACP** — 外部ツールサーバーに接続、または Hermes をツールサーバーとして公開：
```yaml
mcp:
  servers:
    - name: my-tools
      command: npx my-mcp-server
```

**ボイスモード** — VAD + STT + TTS パイプラインでハンズフリー操作。

## アーキテクチャ

```
hermes-cli                    # バイナリエントリポイント、TUI、スラッシュコマンド
├── hermes-agent              # エージェントループ、LLM プロバイダ、メモリプラグイン
│   ├── hermes-core           # 共有型、trait、エラー階層
│   ├── hermes-intelligence   # モデルルーティング、プロンプト構築、自己進化
│   └── hermes-config         # 設定読み込み、YAML/環境変数マージ
├── hermes-tools              # 30 以上のツールバックエンド、承認エンジン
├── hermes-gateway            # 17 プラットフォームアダプタ、セッション管理
├── hermes-environments       # ターミナル: Local/Docker/SSH/Daytona/Modal/Singularity
├── hermes-mcp                # Model Context Protocol クライアント/サーバー
├── hermes-acp                # Agent Communication Protocol
├── hermes-skills             # スキル管理と Hub
├── hermes-cron               # Cron スケジューリング
├── hermes-dashboard          # Web ダッシュボード + HTTP/WebSocket API サーバー
├── hermes-auth               # OAuth トークン交換
├── hermes-eval               # SWE-bench、Terminal-Bench、YC Bench
└── hermes-telemetry          # OpenTelemetry + Prometheus
```

**主要 trait：** `LlmProvider`（10 プロバイダ）· `ToolHandler`（30 以上のバックエンド）· `PlatformAdapter`（17 プラットフォーム）· `TerminalBackend`（6 バックエンド）· `MemoryProvider`（8 プラグイン）

**ツールコールパーサー：** Hermes、Anthropic、OpenAI、Qwen、Llama、DeepSeek、Auto

## インストール

**ワンライナー**（OS と CPU を自動検出）：
```bash
curl -fsSL https://raw.githubusercontent.com/Lumio-Research/hermes-agent-rs/main/scripts/install.sh | bash
```

**ソースから：**
```bash
cargo install --git https://github.com/Lumio-Research/hermes-agent-rs hermes-cli --locked
```

**手動ダウンロード：** [Releases](https://github.com/Lumio-Research/hermes-agent-rs/releases) からプラットフォーム別バイナリを取得。

**Docker：**
```bash
docker run --rm -it -v ~/.hermes:/root/.hermes ghcr.io/lumio-research/hermes-agent-rs
```

## コントリビュート

コントリビュート歓迎。提出前にテストを実行してください：

```bash
cargo test --workspace        # 1,428 テスト
cargo clippy --workspace      # リント
cargo fmt --all --check       # フォーマット
```

アーキテクチャの詳細とコーディング規約は [AGENTS.md](AGENTS.md) を参照。

## ライセンス

MIT — [LICENSE](LICENSE) を参照。

[Nous Research](https://nousresearch.com) の [Hermes Agent](https://github.com/NousResearch/hermes-agent) に基づく。
