use async_trait::async_trait;
use thiserror::Error;

use crate::messages::BusMessage;

#[derive(Debug, Error)]
pub enum BusError {
    #[error("transport closed")]
    Closed,
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("timeout")]
    Timeout,
}

#[async_trait]
pub trait BusTransport: Send + Sync {
    async fn send(&self, message: BusMessage) -> Result<(), BusError>;
    async fn receive(&self) -> Result<BusMessage, BusError>;
}
