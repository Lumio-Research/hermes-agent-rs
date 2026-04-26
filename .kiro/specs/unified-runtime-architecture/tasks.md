# Implementation Plan: Unified Runtime Architecture

## Overview

This plan implements a phased refactoring of the Hermes Agent system across three phases: (1) AgentService trait + Dashboard decoupling, (2) hermes-runtime + unified serve command, and (3) message bus architecture stubs. Each task builds incrementally on previous work, with checkpoints to verify correctness after each major milestone.

## Tasks

- [x] 1. Define AgentService trait and supporting types in hermes-core
  - [x] 1.1 Add AgentService trait, AgentOverrides, and AgentReply to `crates/hermes-core/src/traits.rs`
    - Define `AgentService` async trait with `send_message`, `send_message_stream`, `get_session_messages`, and `reset_session` methods
    - Define `AgentOverrides` struct with `model: Option<String>` and `personality: Option<String>`, deriving `Debug, Clone, Default, Serialize, Deserialize`
    - Define `AgentReply` struct with `text: String` and `message_count: usize`, deriving `Debug, Clone, Serialize, Deserialize`
    - Add `Send + Sync` bounds on the trait
    - Re-export `AgentService`, `AgentOverrides`, `AgentReply` from `crates/hermes-core/src/lib.rs`
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6_

- [x] 2. Extract shared agent construction functions into hermes-agent
  - [x] 2.1 Create `crates/hermes-agent/src/agent_builder.rs` with shared functions
    - Move `build_agent_config`, `build_provider`, `bridge_tool_registry`, and `provider_api_key_from_env` from `crates/hermes-cli/src/app.rs` to `crates/hermes-agent/src/agent_builder.rs`
    - Add `pub mod agent_builder;` to `crates/hermes-agent/src/lib.rs`
    - The `build_agent_config` function should accept a `platform: Option<&str>` parameter so callers can specify "cli", "http", etc.
    - Include the `StubProvider` fallback from `app.rs`
    - _Requirements: 12.1, 12.2_

  - [x] 2.2 Refactor `crates/hermes-cli/src/app.rs` to import from `hermes_agent::agent_builder`
    - Replace local `build_agent_config`, `build_provider`, `bridge_tool_registry`, `provider_api_key_from_env` with imports from `hermes_agent::agent_builder`
    - Keep the re-exports (`pub use hermes_agent::agent_builder::{...}`) so that `hermes-cli/src/main.rs` imports remain unchanged
    - Remove the `StubProvider` struct from `app.rs`
    - _Requirements: 12.4_

  - [x] 2.3 Refactor `crates/hermes-dashboard/src/lib.rs` to import from `hermes_agent::agent_builder`
    - Replace local `build_agent_config`, `build_provider`, `bridge_tool_registry` with imports from `hermes_agent::agent_builder`
    - Delete the duplicate function bodies from `lib.rs`
    - Keep `resolve_model_for_gateway`, `build_agent_for_gateway_context`, `resolve_model`, and `extract_last_assistant_reply` in `lib.rs` (they are dashboard-specific)
    - _Requirements: 12.3_

  - [x]* 2.4 Write unit tests for shared agent builder functions
    - Test `build_agent_config` produces correct `AgentConfig` fields from a `GatewayConfig`
    - Test `build_provider` returns correct provider type for each known provider name
    - Test `bridge_tool_registry` bridges all tools from `hermes_tools::ToolRegistry`
    - Test `provider_api_key_from_env` returns correct env var names
    - _Requirements: 12.1, 12.2_

- [x] 3. Checkpoint — Verify shared builder extraction
  - Ensure `cargo build --workspace` compiles cleanly
  - Ensure `cargo test --workspace` passes with zero new failures
  - Ensure `cargo clippy --workspace --all-targets` reports zero new warnings
  - Ensure all tests pass, ask the user if questions arise.

- [x] 4. Implement LocalAgentService in hermes-agent
  - [x] 4.1 Create `crates/hermes-agent/src/local_agent_service.rs`
    - Implement `LocalAgentService` struct holding `config: Arc<GatewayConfig>`, `tool_registry: Arc<ToolRegistry>`, `session_persistence: Arc<SessionPersistence>`
    - Implement `AgentService` trait for `LocalAgentService`
    - `send_message`: load session messages from `SessionPersistence`, append user message, build `AgentConfig` + provider via shared functions, create `AgentLoop`, call `.run(messages)`, persist updated messages, return `AgentReply`
    - `send_message_stream`: same flow but use `.run_stream()` with `on_chunk` callback
    - `get_session_messages`: delegate to `session_persistence.load_session()`
    - `reset_session`: delete session messages from SQLite
    - Add `pub mod local_agent_service;` to `crates/hermes-agent/src/lib.rs`
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6_

  - [x]* 4.2 Write property test for session persistence round-trip
    - **Property 1: Session persistence round-trip**
    - Generate random `Vec<Message>` with varying roles, content (empty, unicode, long strings), tool_calls, and tool_call_ids
    - Persist to a temp SQLite via `SessionPersistence`, then load and verify roles, content, tool_call_ids, and tool_calls match
    - Place in `crates/hermes-agent/tests/prop_session_persistence.rs`
    - **Validates: Requirements 1.3, 2.2, 5.2**

  - [x]* 4.3 Write property test for session count
    - **Property 2: Session count matches persisted sessions**
    - Generate N distinct session IDs (0..50), persist messages to each, query count from SQLite, verify equals N
    - Place in `crates/hermes-agent/tests/prop_session_persistence.rs`
    - **Validates: Requirements 4.4**

  - [x]* 4.4 Write property test for send_message flow
    - **Property 3: send_message appends user message and assistant reply**
    - Generate random session IDs and message texts, use a mock `LlmProvider` returning deterministic replies
    - Call `LocalAgentService.send_message`, verify session contains user message + assistant reply
    - Place in `crates/hermes-agent/tests/prop_agent_service.rs`
    - **Validates: Requirements 2.3**

  - [x]* 4.5 Write unit tests for LocalAgentService
    - Test `send_message` with mock LLM provider returns correct `AgentReply`
    - Test `send_message_stream` forwards chunks to callback
    - Test `get_session_messages` returns persisted messages
    - Test `reset_session` clears session history
    - _Requirements: 13.4_

- [x] 5. Decouple Dashboard from Gateway
  - [x] 5.1 Update `HttpServerState` to use `Arc<dyn AgentService>` instead of `Arc<Gateway>`
    - Replace `gateway: Arc<Gateway>` and `outbound: ChatOutboundBuffer` fields with `agent_service: Arc<dyn AgentService>` in `crates/hermes-dashboard/src/lib.rs`
    - Update `HttpServerState::build` to create a `LocalAgentService` and store it as `Arc<dyn AgentService>`
    - Remove `HttpPlatformAdapter` and `ChatOutboundBuffer` structs
    - _Requirements: 3.1, 3.5, 3.6_

  - [x] 5.2 Refactor `/v1/sessions/{session_id}/messages` endpoint to use AgentService
    - Update `send_message` handler to call `agent_service.send_message()` instead of routing through Gateway
    - Update `exec_command` handler similarly
    - _Requirements: 3.3_

  - [x] 5.3 Refactor WebSocket endpoints to use AgentService
    - Update `handle_ws` to use `agent_service.send_message()` instead of Gateway routing
    - Update `handle_ws_stream` to use `agent_service.send_message_stream()` instead of constructing a standalone `AgentLoop`
    - _Requirements: 3.4_

  - [x] 5.4 Update status endpoint to use PID file instead of Gateway
    - Modify `crates/hermes-dashboard/src/dashboard/status.rs` to read `$HERMES_HOME/gateway.pid` and probe the process with `kill(pid, 0)` (Unix) or equivalent
    - Query active session count from `SessionPersistence` via `SELECT COUNT(DISTINCT id) FROM sessions`
    - Remove all `gateway.adapter_names()` and `gateway.session_manager()` calls
    - Report `gateway_pid` from the PID file
    - When no PID file exists or PID is dead, report `gateway_running: false`, `gateway_state: null`
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_

  - [x] 5.5 Verify session endpoints query SQLite directly
    - Confirm `crates/hermes-dashboard/src/dashboard/sessions.rs` already queries `SessionPersistence` directly
    - Remove any remaining Gateway references from session endpoints
    - Ensure session delete endpoint deletes from `SessionPersistence` directly
    - _Requirements: 5.1, 5.2, 5.3_

  - [x] 5.6 Remove `hermes-gateway` dependency from `crates/hermes-dashboard/Cargo.toml`
    - Delete `hermes-gateway = { workspace = true }` from `[dependencies]`
    - Remove all `use hermes_gateway::*` imports from dashboard source files
    - _Requirements: 3.2_

  - [x]* 5.7 Write property test for status response schema stability
    - **Property 7: REST API response schema stability**
    - Generate random hermes_home paths and session counts, construct status response, verify all required fields present with correct types
    - Place in `crates/hermes-dashboard/tests/prop_status_schema.rs`
    - **Validates: Requirements 9.3**

- [x] 6. Checkpoint — Phase 1 complete
  - Ensure `cargo build --workspace` compiles cleanly ✓
  - Ensure `cargo test --workspace` passes with zero new failures ✓ (one test skipped as it requires API keys)
  - Ensure `cargo clippy --workspace --all-targets` reports zero new warnings ✓
  - Verify Dashboard runs without Gateway: `hermes web` starts and `/api/status` returns valid JSON ✓
  - Ensure all tests pass, ask the user if questions arise.

- [x] 7. Extract PlatformRegistry into hermes-gateway
  - [x] 7.1 Create `crates/hermes-gateway/src/platform_registry.rs`
    - Define `RegistrationSummary` struct with `registered: Vec<String>` and `errors: Vec<(String, String)>`
    - Implement `pub async fn register_platforms(gateway: &Gateway, config: &GatewayConfig) -> Result<RegistrationSummary, AgentError>`
    - Move the ~500 lines of adapter construction code from `crates/hermes-cli/src/main.rs` `run_gateway` into this function
    - Support all 17 platform adapters + ApiServer + Webhook
    - Add `pub mod platform_registry;` to `crates/hermes-gateway/src/lib.rs`
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5_

  - [x] 7.2 Refactor `run_gateway` in `crates/hermes-cli/src/main.rs` to use PlatformRegistry
    - Replace inline platform construction with a call to `hermes_gateway::platform_registry::register_platforms()`
    - Log the `RegistrationSummary` (registered adapters and errors)
    - _Requirements: 6.6_

  - [x]* 7.3 Write property test for platform registration
    - **Property 4: Platform registration matches enabled config**
    - Generate random `HashMap<String, PlatformConfig>` with random `enabled` flags
    - Call `register_platforms` with mock Gateway, verify registered names match enabled set
    - Place in `crates/hermes-gateway/tests/prop_platform_registry.rs`
    - **Validates: Requirements 6.2, 6.3, 6.5, 13.5**

  - [x]* 7.4 Write unit tests for PlatformRegistry
    - Test with all platforms disabled → empty registration
    - Test with specific platforms enabled → only those registered
    - Test with missing tokens → error in summary, other platforms still registered
    - _Requirements: 13.5_

- [x] 8. Create hermes-runtime crate with RuntimeBuilder
  - [x] 8.1 Create `crates/hermes-runtime/` crate scaffold
    - Create `crates/hermes-runtime/Cargo.toml` with dependencies on `hermes-core`, `hermes-agent`, `hermes-gateway`, `hermes-dashboard`, `hermes-cron`, `hermes-config`, `hermes-tools`, `hermes-skills`, `hermes-environments`, `tokio`
    - Add `hermes-runtime = { path = "crates/hermes-runtime" }` to workspace `[workspace.dependencies]` in root `Cargo.toml`
    - Add `"crates/hermes-runtime"` to workspace `members` in root `Cargo.toml`
    - _Requirements: 7.1_

  - [x] 8.2 Implement RuntimeBuilder in `crates/hermes-runtime/src/lib.rs`
    - Define `RuntimeBuilder` struct with `config: GatewayConfig`, `dashboard_addr: Option<SocketAddr>`, `enable_platforms: bool`, `enable_cron: bool`
    - Implement builder methods: `new(config)`, `with_dashboard(addr)`, `with_platforms()`, `with_cron()`
    - Implement `pub async fn run(self) -> Result<(), AgentError>` that:
      1. Builds shared `LocalAgentService`
      2. Optionally starts Dashboard via `HttpServerState::build` with the shared `AgentService`
      3. Optionally registers platforms via `PlatformRegistry`
      4. Optionally starts `CronScheduler`
      5. Uses `tokio::select!` to run all subsystems + handle ctrl_c shutdown
    - Ensure all subsystems share the same `Arc<dyn AgentService>` instance
    - _Requirements: 7.2, 7.3, 7.4, 7.5, 7.6, 7.7, 7.8_

  - [x]* 8.3 Write unit tests for RuntimeBuilder
    - Test builder pattern: `with_dashboard`, `with_platforms`, `with_cron` set correct flags
    - Test that all subsystems share the same `AgentService` instance (Arc pointer equality)
    - _Requirements: 13.6_

- [x] 9. Add `hermes serve` CLI command
  - [x] 9.1 Add `Serve` subcommand to `crates/hermes-cli/src/cli.rs`
    - Add `Serve { host, port, no_dashboard, no_platforms, no_cron }` variant to `CliCommand` enum
    - Define `--host` (default "0.0.0.0"), `--port` (default 3000), `--no-dashboard`, `--no-platforms`, `--no-cron` flags
    - _Requirements: 8.1_

  - [x] 9.2 Implement `run_serve` handler in `crates/hermes-cli/src/main.rs`
    - Add `hermes-runtime` dependency to `crates/hermes-cli/Cargo.toml`
    - Implement `run_serve` that creates a `RuntimeBuilder` from config, applies flags, and calls `.run()`
    - Wire `CliCommand::Serve` dispatch in `main()`
    - _Requirements: 8.2, 8.3, 8.4, 8.5, 8.6_

  - [x] 9.3 Refactor `run_gateway` and `run_web` to use RuntimeBuilder
    - `run_gateway` becomes `RuntimeBuilder::new(config).with_platforms().with_cron().run()`
    - `run_web` becomes `RuntimeBuilder::new(config).with_dashboard(addr).run()`
    - Preserve existing CLI argument handling and output messages
    - _Requirements: 9.1, 9.2_

- [x] 10. Checkpoint — Phase 2 complete
  - Ensure `cargo build --workspace` compiles cleanly
  - Ensure `cargo test --workspace` passes with zero new failures
  - Ensure `cargo clippy --workspace --all-targets` reports zero new warnings
  - Verify `hermes serve` starts Dashboard + platforms + cron
  - Verify `hermes gateway start` and `hermes web` still work (backward compatibility)
  - Ensure all tests pass, ask the user if questions arise.

- [x] 11. Create hermes-bus crate with message types and transport
  - [x] 11.1 Create `crates/hermes-bus/` crate scaffold
    - Create `crates/hermes-bus/Cargo.toml` with dependencies on `hermes-core`, `serde`, `serde_json`, `tokio`, `async-trait`, `thiserror`
    - Add `hermes-bus = { path = "crates/hermes-bus" }` to workspace `[workspace.dependencies]` in root `Cargo.toml`
    - Add `"crates/hermes-bus"` to workspace `members` in root `Cargo.toml`
    - _Requirements: 10.1_

  - [x] 11.2 Define message types in `crates/hermes-bus/src/messages.rs`
    - Define `BusMessage` enum with variants: `AgentRequest`, `AgentResponse`, `PlatformIncoming`, `PlatformOutgoing`, `SessionQuery`, `SessionResponse`, `CronTrigger`, `StatusQuery`
    - Define all supporting structs (`AgentRequest`, `AgentResponse`, `PlatformIncoming`, `PlatformOutgoing`, `SessionQuery`, `SessionResponse`, `SessionSummary`, `CronTrigger`, `StatusQuery`)
    - All types derive `Debug, Clone, Serialize, Deserialize`
    - _Requirements: 10.2, 10.5_

  - [x] 11.3 Define BusTransport trait and BusError in `crates/hermes-bus/src/transport.rs`
    - Define `BusError` enum with `Closed`, `Serialization(String)`, `Timeout` variants using `thiserror`
    - Define `BusTransport` async trait with `send(&self, BusMessage)` and `receive(&self)` methods
    - _Requirements: 10.3_

  - [x] 11.4 Implement InProcessTransport in `crates/hermes-bus/src/in_process.rs`
    - Implement `InProcessTransport` using `tokio::sync::mpsc` channels
    - Provide `InProcessTransport::new(buffer_size)` constructor that returns a `(InProcessTransport, InProcessTransport)` sender/receiver pair
    - Implement `BusTransport` for `InProcessTransport`
    - _Requirements: 10.4_

  - [x] 11.5 Wire up `crates/hermes-bus/src/lib.rs` with module declarations and re-exports
    - Add `pub mod messages;`, `pub mod transport;`, `pub mod in_process;`, `pub mod remote_agent_service;`
    - Re-export key types: `BusMessage`, `BusTransport`, `BusError`, `InProcessTransport`, `RemoteAgentService`
    - _Requirements: 10.1_

  - [x]* 11.6 Write property test for bus message serde round-trip
    - **Property 5: Bus message serde round-trip**
    - Generate random `BusMessage` variants with arbitrary field values
    - Serialize to JSON, deserialize back, verify equality
    - Place in `crates/hermes-bus/tests/prop_bus_messages.rs`
    - **Validates: Requirements 10.5**

  - [x]* 11.7 Write property test for InProcessTransport round-trip
    - **Property 6: InProcessTransport send-receive round-trip**
    - Generate random `BusMessage`, send through `InProcessTransport`, receive, verify equality
    - Place in `crates/hermes-bus/tests/prop_bus_transport.rs`
    - **Validates: Requirements 10.4**

- [x] 12. Implement RemoteAgentService stub in hermes-bus
  - [x] 12.1 Create `crates/hermes-bus/src/remote_agent_service.rs`
    - Define `RemoteAgentService` struct holding `transport: Arc<dyn BusTransport>`
    - Implement `AgentService` trait: `send_message` serializes `AgentRequest`, sends via transport, awaits `AgentResponse`
    - `send_message_stream` delegates to `send_message` (streaming not yet supported over bus)
    - `get_session_messages` sends `SessionQuery`, awaits `SessionResponse`
    - `reset_session` sends appropriate message
    - Convert `BusError` to `AgentError::Io` for callers
    - _Requirements: 11.1, 11.2, 11.3, 11.4_

- [x] 13. Final checkpoint — All phases complete
  - Ensure `cargo build --workspace` compiles cleanly
  - Ensure `cargo test --workspace` passes with zero new failures
  - Ensure `cargo clippy --workspace --all-targets` reports zero new warnings
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation after each phase
- Property tests validate universal correctness properties from the design document
- Unit tests validate specific examples and edge cases
- Phase 1 (tasks 1–6) is the foundation — must be completed first
- Phase 2 (tasks 7–10) depends on Phase 1 completion
- Phase 3 (tasks 11–13) can be done independently after Phase 1, as it only defines interfaces
