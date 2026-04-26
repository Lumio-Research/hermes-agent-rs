use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::messages::BusMessage;
use crate::transport::{BusError, BusTransport};

#[derive(Debug, Clone)]
pub struct InProcessTransport {
    tx: tokio::sync::mpsc::Sender<BusMessage>,
    rx: Arc<Mutex<tokio::sync::mpsc::Receiver<BusMessage>>>,
}

impl InProcessTransport {
    pub fn new(buffer_size: usize) -> (Self, Self) {
        let (a_tx, a_rx) = tokio::sync::mpsc::channel(buffer_size);
        let (b_tx, b_rx) = tokio::sync::mpsc::channel(buffer_size);

        let a = Self {
            tx: a_tx,
            rx: Arc::new(Mutex::new(b_rx)),
        };
        let b = Self {
            tx: b_tx,
            rx: Arc::new(Mutex::new(a_rx)),
        };
        (a, b)
    }
}

#[async_trait]
impl BusTransport for InProcessTransport {
    async fn send(&self, message: BusMessage) -> Result<(), BusError> {
        self.tx.send(message).await.map_err(|_| BusError::Closed)
    }

    async fn receive(&self) -> Result<BusMessage, BusError> {
        let mut guard = self.rx.lock().await;
        guard.recv().await.ok_or(BusError::Closed)
    }
}
