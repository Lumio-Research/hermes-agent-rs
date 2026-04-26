<div align="center">

# ⚡ Hermes Agent

**스스로 진화하는 AI 에이전트. 바이너리 하나로 모든 플랫폼.**

[Nous Research](https://nousresearch.com)의 [Hermes Agent](https://github.com/NousResearch/hermes-agent)를 Rust로 재작성한 프로덕션 버전.

`110,000줄 이상의 Rust` · `1,428개 테스트` · `17개 크레이트` · `~16MB 바이너리`

**[English](./README.md)** · **[中文](./README_ZH.md)** · **[日本語](./README_JA.md)** · **[한국어](./README_KO.md)**

</div>

---

## 왜 Hermes인가?

🚀 **의존성 제로** — 단일 정적 바이너리. Python, pip, Docker 필요 없음. Raspberry Pi, 월 $3 VPS, 에어갭 서버에 복사해서 바로 실행.

🧠 **자기 진화 엔진** — 멀티 암드 밴딧 모델 선택, 장기 태스크 계획, 프롬프트·메모리 자동 최적화. 쓸수록 에이전트가 똑똑해진다.

🔌 **17개 플랫폼 · 30개 이상의 도구 · 8개 메모리 백엔드** — Telegram, Discord, Slack, WhatsApp, Signal, Matrix 외 11개 플랫폼. 파일 작업, 브라우저, 코드 실행, 비전, 음성, 웹 검색, Home Assistant 등.

⚡ **진정한 동시성** — Rust의 tokio 런타임이 도구 호출을 OS 스레드에 분산. 30초짜리 브라우저 스크래핑이 50ms 파일 읽기를 차단하지 않음. GIL 없음.

## 빠른 시작

```bash
# 설치
curl -fsSL https://raw.githubusercontent.com/Lumio-Research/hermes-agent-rs/main/scripts/install.sh | bash

# API 키 설정
echo "ANTHROPIC_API_KEY=sk-..." >> ~/.hermes/.env

# 실행
hermes
```

끝. 도구, 메모리, 스트리밍 출력이 갖춰진 대화형 세션이 시작된다.

## 무엇을 할 수 있나?

**어떤 LLM과도 대화** — 대화 중 모델 전환:
```
hermes
> /model gpt-4o
> 이 저장소를 분석하고 보안 문제를 찾아줘
```

**커맨드라인에서 원샷 실행**:
```bash
hermes chat --query "auth.rs를 새로운 에러 타입으로 리팩토링해줘"
```

**멀티 플랫폼 게이트웨이** — Telegram, Discord, Slack 등을 동시 연결:
```bash
hermes gateway start
```

**어디서든 실행** — Docker, SSH, 원격 샌드박스:
```yaml
# ~/.hermes/config.yaml
terminal:
  backend: docker
  image: ubuntu:24.04
```

**MCP + ACP** — 외부 도구 서버에 연결하거나 Hermes를 도구 서버로 노출:
```yaml
mcp:
  servers:
    - name: my-tools
      command: npx my-mcp-server
```

**보이스 모드** — VAD + STT + TTS 파이프라인으로 핸즈프리 사용.

## 아키텍처

```
hermes-cli                    # 바이너리 진입점, TUI, 슬래시 명령
├── hermes-agent              # 에이전트 루프, LLM 프로바이더, 메모리 플러그인
│   ├── hermes-core           # 공유 타입, trait, 에러 계층
│   ├── hermes-intelligence   # 모델 라우팅, 프롬프트 구축, 자기 진화
│   └── hermes-config         # 설정 로딩, YAML/환경변수 병합
├── hermes-tools              # 30개 이상의 도구 백엔드, 승인 엔진
├── hermes-gateway            # 17개 플랫폼 어댑터, 세션 관리
├── hermes-environments       # 터미널: Local/Docker/SSH/Daytona/Modal/Singularity
├── hermes-mcp                # Model Context Protocol 클라이언트/서버
├── hermes-acp                # Agent Communication Protocol
├── hermes-skills             # 스킬 관리 및 Hub
├── hermes-cron               # Cron 스케줄링
├── hermes-dashboard          # 웹 대시보드 + HTTP/WebSocket API 서버
├── hermes-auth               # OAuth 토큰 교환
├── hermes-eval               # SWE-bench, Terminal-Bench, YC Bench
└── hermes-telemetry          # OpenTelemetry + Prometheus
```

**핵심 trait:** `LlmProvider` (10개 프로바이더) · `ToolHandler` (30개 이상 백엔드) · `PlatformAdapter` (17개 플랫폼) · `TerminalBackend` (6개 백엔드) · `MemoryProvider` (8개 플러그인)

**도구 호출 파서:** Hermes, Anthropic, OpenAI, Qwen, Llama, DeepSeek, Auto

## 설치

**원라인 설치** (OS와 CPU 자동 감지):
```bash
curl -fsSL https://raw.githubusercontent.com/Lumio-Research/hermes-agent-rs/main/scripts/install.sh | bash
```

**소스에서 설치:**
```bash
cargo install --git https://github.com/Lumio-Research/hermes-agent-rs hermes-cli --locked
```

**수동 다운로드:** [Releases](https://github.com/Lumio-Research/hermes-agent-rs/releases)에서 플랫폼별 바이너리 다운로드.

**Docker:**
```bash
docker run --rm -it -v ~/.hermes:/root/.hermes ghcr.io/lumio-research/hermes-agent-rs
```

## 기여하기

기여를 환영합니다. 제출 전에 테스트를 실행해 주세요:

```bash
cargo test --workspace        # 1,428개 테스트
cargo clippy --workspace      # 린트
cargo fmt --all --check       # 포맷
```

아키텍처 상세와 코딩 규칙은 [AGENTS.md](AGENTS.md)를 참고하세요.

## 라이선스

MIT — [LICENSE](LICENSE) 참조.

[Nous Research](https://nousresearch.com)의 [Hermes Agent](https://github.com/NousResearch/hermes-agent) 기반.
