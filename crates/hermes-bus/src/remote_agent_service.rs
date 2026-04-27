use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use hermes_core::traits::{AgentOverrides, AgentReply, AgentService};
use hermes_core::{AgentError, Message, StreamChunk};

use crate::messages::{AgentRequest, BusMessage, SessionQuery, SessionQueryAction};
use crate::transport::{BusError, BusTransport};

static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub struct RemoteAgentService {
    transport: Arc<dyn BusTransport>,
}

impl RemoteAgentService {
    pub fn new(transport: Arc<dyn BusTransport>) -> Self {
        Self { transport }
    }

    fn bus_to_agent_error(err: BusError) -> AgentError {
        AgentError::Io(err.to_string())
    }

    fn next_request_id() -> String {
        format!("req-{}", REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

#[async_trait]
impl AgentService for RemoteAgentService {
    async fn send_message(
        &self,
        session_id: &str,
        text: &str,
        overrides: AgentOverrides,
    ) -> Result<AgentReply, AgentError> {
        let request_id = Self::next_request_id();
        let req = AgentRequest {
            request_id: request_id.clone(),
            session_id: session_id.to_string(),
            text: text.to_string(),
            overrides,
            stream: false,
        };
        self.transport
            .send(BusMessage::AgentRequest(req))
            .await
            .map_err(Self::bus_to_agent_error)?;

        loop {
            match self
                .transport
                .receive()
                .await
                .map_err(Self::bus_to_agent_error)?
            {
                BusMessage::AgentResponse(resp) if resp.request_id == request_id && resp.done => {
                    if let Some(err) = resp.error {
                        return Err(AgentError::Io(err));
                    }
                    return Ok(AgentReply {
                        text: resp.text,
                        message_count: resp.message_count,
                    });
                }
                BusMessage::AgentStreamChunk(_chunk) => {
                    // Non-stream request: ignore stream fragments.
                }
                _ => {}
            }
        }
    }

    async fn send_message_stream(
        &self,
        session_id: &str,
        text: &str,
        overrides: AgentOverrides,
        on_chunk: Arc<dyn Fn(StreamChunk) + Send + Sync>,
    ) -> Result<AgentReply, AgentError> {
        let request_id = Self::next_request_id();
        let req = AgentRequest {
            request_id: request_id.clone(),
            session_id: session_id.to_string(),
            text: text.to_string(),
            overrides,
            stream: true,
        };
        self.transport
            .send(BusMessage::AgentRequest(req))
            .await
            .map_err(Self::bus_to_agent_error)?;

        loop {
            match self
                .transport
                .receive()
                .await
                .map_err(Self::bus_to_agent_error)?
            {
                BusMessage::AgentStreamChunk(ev) if ev.request_id == request_id => {
                    on_chunk(ev.chunk);
                }
                BusMessage::AgentResponse(resp) if resp.request_id == request_id && resp.done => {
                    if let Some(err) = resp.error {
                        return Err(AgentError::Io(err));
                    }
                    return Ok(AgentReply {
                        text: resp.text,
                        message_count: resp.message_count,
                    });
                }
                _ => {}
            }
        }
    }

    async fn get_session_messages(&self, session_id: &str) -> Result<Vec<Message>, AgentError> {
        let request_id = Self::next_request_id();
        let query = SessionQuery {
            request_id: request_id.clone(),
            session_id: session_id.to_string(),
            action: SessionQueryAction::GetMessages,
        };
        self.transport
            .send(BusMessage::SessionQuery(query))
            .await
            .map_err(Self::bus_to_agent_error)?;

        loop {
            match self
                .transport
                .receive()
                .await
                .map_err(Self::bus_to_agent_error)?
            {
                BusMessage::SessionResponse(resp) if resp.request_id == request_id => {
                    if let Some(err) = resp.error {
                        return Err(AgentError::Io(err));
                    }
                    return Ok(resp.messages);
                }
                _ => {}
            }
        }
    }

    async fn reset_session(&self, session_id: &str) -> Result<(), AgentError> {
        let request_id = Self::next_request_id();
        let query = SessionQuery {
            request_id: request_id.clone(),
            session_id: session_id.to_string(),
            action: SessionQueryAction::ResetSession,
        };
        self.transport
            .send(BusMessage::SessionQuery(query))
            .await
            .map_err(Self::bus_to_agent_error)?;
        loop {
            match self
                .transport
                .receive()
                .await
                .map_err(Self::bus_to_agent_error)?
            {
                BusMessage::SessionResponse(resp) if resp.request_id == request_id => {
                    if let Some(err) = resp.error {
                        return Err(AgentError::Io(err));
                    }
                    return Ok(());
                }
                _ => {}
            }
        }
    }
}
