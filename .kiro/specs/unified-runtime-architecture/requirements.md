# Requirements Document

## Introduction

This document specifies the requirements for a phased architecture refactoring of the Hermes Agent system. The current architecture tightly couples the Dashboard to the Gateway, embeds platform registration logic in the CLI binary, and runs all components in a single process. This refactoring introduces an `AgentService` abstraction, a unified runtime crate, and a message bus to decouple components, enable independent scaling, and improve fault isolation.

The work is organized into three phases:
- **Phase 1**: AgentService trait + Dashboard decoupling from Gateway
- **Phase 2**: hermes-runtime crate + unified `hermes serve` command
- **Phase 3**: Message bus + process separation (future-ready architecture)

## Glossary

- **AgentService**: An async trait that abstracts agent execution (sending messages, receiving replies), allowing callers to be agnostic about whether the agent runs in-process or remotely.
- **LocalAgentService**: An in-process implementation of AgentService that wraps AgentLoop, ToolRegistry, and an LLM provider directly.
- **RemoteAgentService**: A future implementation of AgentService that communicates with an agent worker over a message bus.
- **AgentLoop**: The core agent engine in `hermes-agent` that orchestrates LLM calls, tool execution, and memory management.
- **Gateway**: The current orchestrator in `hermes-gateway` that manages platform adapters, session state, DM authorization, and message routing.
- **Dashboard**: The HTTP/WebSocket API server in `hermes-dashboard` that serves the web management UI and REST endpoints.
- **RuntimeBuilder**: A builder-pattern struct in `hermes-runtime` that composes Dashboard, platform adapters, and cron into a single runtime.
- **Platform_Adapter**: A trait implementation for a specific messaging platform (Telegram, Discord, WeChat, etc.).
- **Platform_Registry**: A module that maps platform configuration entries to their corresponding Platform_Adapter constructors.
- **Message_Bus**: An inter-process communication layer using Unix sockets or TCP for routing typed messages between components.
- **Hermes_Home**: The user configuration directory (typically `~/.hermes/`), resolved via `hermes_config::hermes_home()`.
- **Session_Persistence**: The SQLite-backed storage for conversation history, managed by `hermes-agent`.
- **Cron_Scheduler**: The timer-based job scheduler in `hermes-cron` that triggers agent tasks on a schedule.

## Requirements

### Requirement 1: AgentService Trait Definition

**User Story:** As a crate author, I want a trait that abstracts agent execution, so that Dashboard and other consumers can use the agent without depending on Gateway or specific LLM provider wiring.

#### Acceptance Criteria

1. THE AgentService trait SHALL define an async method `send_message` that accepts a session identifier, a user message string, and optional model/personality overrides, and returns a result containing the agent reply text.
2. THE AgentService trait SHALL define an async method `send_message_stream` that accepts the same parameters as `send_message` and returns a stream of text chunks followed by a final reply.
3. THE AgentService trait SHALL define an async method `get_session_messages` that accepts a session identifier and returns the list of messages for that session.
4. THE AgentService trait SHALL define an async method `reset_session` that accepts a session identifier and clears the session message history.
5. THE AgentService trait SHALL be defined in the `hermes-core` crate so that all downstream crates can depend on the abstraction without circular dependencies.
6. THE AgentService trait SHALL require `Send + Sync` bounds so that implementations are safe to share across async tasks.

### Requirement 2: LocalAgentService Implementation

**User Story:** As a developer, I want an in-process AgentService implementation, so that Dashboard and TUI can run the agent directly without Gateway overhead.

#### Acceptance Criteria

1. THE LocalAgentService SHALL implement the AgentService trait by delegating to an AgentLoop instance, a ToolRegistry, and an LLM provider.
2. THE LocalAgentService SHALL manage session message history using Session_Persistence (SQLite) for conversation storage.
3. WHEN `send_message` is called, THE LocalAgentService SHALL append the user message to the session history, invoke AgentLoop.run with the session messages, persist the updated history, and return the assistant reply.
4. WHEN `send_message_stream` is called, THE LocalAgentService SHALL invoke AgentLoop.run_stream and forward streaming chunks to the caller.
5. THE LocalAgentService SHALL accept an AgentConfig, a ToolRegistry, and an LLM provider at construction time, following the same builder patterns used in `hermes-cli/src/app.rs`.
6. THE LocalAgentService SHALL be defined in the `hermes-agent` crate alongside AgentLoop.

### Requirement 3: Dashboard Decoupling from Gateway

**User Story:** As a maintainer, I want the Dashboard to depend on AgentService instead of Gateway, so that the Dashboard can run without platform adapters and the `hermes-gateway` dependency can be removed from `hermes-dashboard`.

#### Acceptance Criteria

1. THE Dashboard SHALL use an AgentService implementation for all agent interactions instead of creating a Gateway instance.
2. THE Dashboard SHALL remove the `hermes-gateway` dependency from its `Cargo.toml`.
3. WHEN the `/v1/sessions/{session_id}/messages` endpoint receives a request, THE Dashboard SHALL delegate to `AgentService.send_message` instead of routing through Gateway.
4. WHEN the `/v1/ws-stream/{session_id}` WebSocket endpoint receives a message, THE Dashboard SHALL delegate to `AgentService.send_message_stream` instead of constructing a standalone AgentLoop.
5. THE Dashboard SHALL accept an `Arc<dyn AgentService>` in its `HttpServerState` struct, replacing the current `Arc<Gateway>` field.
6. WHEN the Dashboard is constructed via `HttpServerState::build`, THE Dashboard SHALL instantiate a LocalAgentService by default.

### Requirement 4: Status Endpoint Independence

**User Story:** As a user, I want the `/api/status` endpoint to work without a running Gateway, so that the Dashboard can report system status independently.

#### Acceptance Criteria

1. THE Dashboard status endpoint SHALL read the Hermes version from the compiled binary metadata (`CARGO_PKG_VERSION`) instead of querying Gateway.
2. THE Dashboard status endpoint SHALL read configuration paths from Hermes_Home instead of querying Gateway.
3. THE Dashboard status endpoint SHALL detect gateway process status by checking the PID file at `Hermes_Home/gateway.pid` and probing the process, instead of querying Gateway adapter state.
4. THE Dashboard status endpoint SHALL query active session count from Session_Persistence (SQLite) instead of from Gateway's SessionManager.
5. WHEN no gateway PID file exists or the PID is not alive, THE Dashboard status endpoint SHALL report `gateway_running: false` and `gateway_state: null`.

### Requirement 5: Session Query Independence

**User Story:** As a user, I want session listing and search endpoints to work without Gateway, so that the Dashboard can browse conversation history independently.

#### Acceptance Criteria

1. THE Dashboard session endpoints (`/api/sessions`, `/api/sessions/search`, `/api/sessions/{session_id}`) SHALL query Session_Persistence (SQLite) directly instead of through Gateway's SessionManager.
2. THE Dashboard session message endpoint (`/api/sessions/{session_id}/messages`) SHALL read messages from Session_Persistence instead of from Gateway's in-memory session store.
3. THE Dashboard session delete endpoint SHALL delete sessions from Session_Persistence directly.

### Requirement 6: Platform Registry Extraction

**User Story:** As a developer, I want platform registration logic extracted into a reusable module, so that both `hermes gateway start` and the future `hermes serve` command can share the same adapter construction code.

#### Acceptance Criteria

1. THE Platform_Registry module SHALL be created in the `hermes-gateway` crate as `platform_registry.rs`.
2. THE Platform_Registry SHALL provide a function `register_platforms` that accepts a Gateway reference and a GatewayConfig reference, and registers all enabled platform adapters.
3. WHEN a platform entry in the configuration has `enabled: true`, THE Platform_Registry SHALL construct the corresponding Platform_Adapter and register it with the Gateway.
4. THE Platform_Registry SHALL support all 17 existing platform adapters (Telegram, Discord, Slack, WhatsApp, Signal, Matrix, Mattermost, DingTalk, Feishu, WeCom, WeComCallback, Weixin, QQBot, BlueBubbles, Email, SMS, HomeAssistant) plus ApiServer and Webhook.
5. WHEN `register_platforms` completes, THE Platform_Registry SHALL return a summary of registered adapter names and any registration errors.
6. THE `run_gateway` function in `hermes-cli/src/main.rs` SHALL be refactored to call `Platform_Registry.register_platforms` instead of containing inline platform construction code.

### Requirement 7: RuntimeBuilder and hermes-runtime Crate

**User Story:** As a developer, I want a unified runtime builder, so that I can compose Dashboard, platform adapters, and cron into a single process with a single command.

#### Acceptance Criteria

1. THE `hermes-runtime` crate SHALL be created as a new workspace member in the root `Cargo.toml`.
2. THE RuntimeBuilder SHALL provide a builder-pattern API with methods `with_dashboard(addr)`, `with_platforms()`, and `with_cron()` that enable respective subsystems.
3. THE RuntimeBuilder SHALL accept a GatewayConfig at construction time.
4. WHEN `with_dashboard` is called, THE RuntimeBuilder SHALL configure the Dashboard HTTP server to bind to the specified address.
5. WHEN `with_platforms` is called, THE RuntimeBuilder SHALL use Platform_Registry to register all enabled platform adapters.
6. WHEN `with_cron` is called, THE RuntimeBuilder SHALL initialize the Cron_Scheduler with the cron data directory from Hermes_Home.
7. THE RuntimeBuilder SHALL provide a `run` method that starts all configured subsystems concurrently using `tokio::select!` and handles graceful shutdown on SIGTERM/SIGINT.
8. THE RuntimeBuilder SHALL ensure that Dashboard, platforms, and cron all share the same AgentService instance.

### Requirement 8: Unified Serve Command

**User Story:** As a user, I want a single `hermes serve` command that starts Dashboard, platform adapters, and cron together, so that I do not need to run `hermes web` and `hermes gateway start` separately.

#### Acceptance Criteria

1. THE CLI SHALL add a `Serve` subcommand with optional flags `--host`, `--port`, `--no-dashboard`, `--no-platforms`, and `--no-cron`.
2. WHEN `hermes serve` is executed without flags, THE CLI SHALL start Dashboard, all enabled platform adapters, and the cron scheduler in a single process.
3. WHEN `--no-dashboard` is specified, THE CLI SHALL start platforms and cron without the Dashboard HTTP server.
4. WHEN `--no-platforms` is specified, THE CLI SHALL start Dashboard and cron without platform adapters.
5. WHEN `--no-cron` is specified, THE CLI SHALL start Dashboard and platforms without the cron scheduler.
6. THE `hermes serve` command SHALL use RuntimeBuilder to compose and start the selected subsystems.

### Requirement 9: Backward Compatibility

**User Story:** As an existing user, I want `hermes gateway start` and `hermes web` to continue working after the refactoring, so that my existing workflows are not broken.

#### Acceptance Criteria

1. WHEN `hermes gateway start` is executed, THE CLI SHALL start the gateway with platform adapters and cron, using the same behavior as before the refactoring.
2. WHEN `hermes web` is executed, THE CLI SHALL start the Dashboard HTTP server on the specified host and port, using AgentService instead of Gateway internally but preserving the same REST/WebSocket API surface.
3. THE REST API response schemas for `/health`, `/v1/sessions/{session_id}/messages`, `/v1/commands`, `/api/status`, `/api/sessions`, and all other existing endpoints SHALL remain unchanged.
4. THE WebSocket protocol for `/v1/ws/{session_id}` and `/v1/ws-stream/{session_id}` SHALL remain unchanged.
5. THE existing `hermes gateway stop` command SHALL continue to work by reading and signaling the PID file.

### Requirement 10: Message Bus Crate (Phase 3 — Architecture Ready)

**User Story:** As an architect, I want the message bus interfaces defined, so that the system is ready for process separation without requiring immediate implementation.

#### Acceptance Criteria

1. THE `hermes-bus` crate SHALL be created as a new workspace member.
2. THE `hermes-bus` crate SHALL define message type enums for: `AgentRequest`, `AgentResponse`, `PlatformIncoming`, `PlatformOutgoing`, `SessionQuery`, `SessionResponse`, `CronTrigger`, and `StatusQuery`.
3. THE `hermes-bus` crate SHALL define a `BusTransport` trait with async methods `send` and `receive` for typed message passing.
4. THE `hermes-bus` crate SHALL provide an `InProcessTransport` implementation that uses `tokio::sync::mpsc` channels for in-process communication.
5. THE message type definitions SHALL use `serde::Serialize` and `serde::Deserialize` so that messages can be serialized for network transport in the future.

### Requirement 11: RemoteAgentService (Phase 3 — Architecture Ready)

**User Story:** As an architect, I want a RemoteAgentService stub defined, so that the system can be extended to support out-of-process agent workers.

#### Acceptance Criteria

1. THE RemoteAgentService SHALL implement the AgentService trait.
2. THE RemoteAgentService SHALL accept a `BusTransport` at construction time.
3. WHEN `send_message` is called, THE RemoteAgentService SHALL serialize an `AgentRequest` message, send it via the BusTransport, and await an `AgentResponse`.
4. THE RemoteAgentService SHALL be defined in the `hermes-bus` crate alongside the transport abstractions.

### Requirement 12: Shared Agent Construction Logic

**User Story:** As a developer, I want agent construction logic (provider selection, tool registration, config bridging) shared between TUI, Dashboard, and Gateway, so that there is no duplication.

#### Acceptance Criteria

1. THE functions `build_agent_config`, `build_provider`, and `bridge_tool_registry` SHALL be moved from `hermes-cli/src/app.rs` to a shared location accessible by both `hermes-cli` and `hermes-dashboard`.
2. THE LocalAgentService SHALL use the shared `build_agent_config`, `build_provider`, and `bridge_tool_registry` functions for agent construction.
3. THE Dashboard SHALL use the shared construction functions instead of maintaining its own duplicate copies in `hermes-dashboard/src/lib.rs`.
4. THE TUI App SHALL use the shared construction functions from the new shared location.

### Requirement 13: Test Continuity

**User Story:** As a developer, I want all existing tests to pass after the refactoring, so that behavioral correctness is preserved.

#### Acceptance Criteria

1. WHEN `cargo test --workspace` is executed after Phase 1 changes, THE test suite SHALL pass with zero new failures.
2. WHEN `cargo test --workspace` is executed after Phase 2 changes, THE test suite SHALL pass with zero new failures.
3. WHEN `cargo clippy --workspace --all-targets` is executed after any phase, THE linter SHALL report zero new warnings.
4. THE AgentService trait SHALL have unit tests verifying that LocalAgentService correctly delegates to AgentLoop for both `send_message` and `send_message_stream`.
5. THE Platform_Registry SHALL have unit tests verifying that enabled platforms are registered and disabled platforms are skipped.
6. THE RuntimeBuilder SHALL have integration tests verifying that subsystems start and stop correctly.
