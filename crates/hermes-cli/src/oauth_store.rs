use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use hermes_core::AgentError;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthCredential {
    pub provider: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String,
    pub scope: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl OAuthCredential {
    pub fn is_expired(&self, leeway_secs: i64) -> bool {
        match self.expires_at {
            Some(exp) => Utc::now() + Duration::seconds(leeway_secs) >= exp,
            None => false,
        }
    }
}

#[derive(Clone)]
pub struct FileTokenStore {
    path: PathBuf,
    cache: Arc<RwLock<HashMap<String, OAuthCredential>>>,
}

impl FileTokenStore {
    pub async fn new(path: impl AsRef<Path>) -> Result<Self, AgentError> {
        let path = path.as_ref().to_path_buf();
        let initial = if tokio::fs::try_exists(&path)
            .await
            .map_err(|e| AgentError::Io(e.to_string()))?
        {
            let raw = tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| AgentError::Io(e.to_string()))?;
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            HashMap::new()
        };
        Ok(Self {
            path,
            cache: Arc::new(RwLock::new(initial)),
        })
    }

    pub async fn get(&self, provider: &str) -> Option<OAuthCredential> {
        self.cache.read().await.get(provider).cloned()
    }

    pub async fn upsert(&self, credential: OAuthCredential) -> Result<(), AgentError> {
        self.cache
            .write()
            .await
            .insert(credential.provider.clone(), credential);
        self.flush().await
    }

    pub async fn remove(&self, provider: &str) -> Result<(), AgentError> {
        self.cache.write().await.remove(provider);
        self.flush().await
    }

    async fn flush(&self) -> Result<(), AgentError> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AgentError::Io(e.to_string()))?;
        }
        let content = serde_json::to_string_pretty(&*self.cache.read().await)
            .map_err(|e| AgentError::Config(e.to_string()))?;
        tokio::fs::write(&self.path, content)
            .await
            .map_err(|e| AgentError::Io(e.to_string()))
    }
}

#[derive(Clone)]
pub struct AuthManager {
    store: FileTokenStore,
}

impl AuthManager {
    pub fn new(store: FileTokenStore) -> Self {
        Self { store }
    }

    pub async fn save_credential(&self, credential: OAuthCredential) -> Result<(), AgentError> {
        self.store.upsert(credential).await
    }

    pub async fn get_access_token(&self, provider: &str) -> Result<Option<String>, AgentError> {
        let Some(credential) = self.store.get(provider).await else {
            return Ok(None);
        };
        if credential.is_expired(30) {
            return Ok(None);
        }
        Ok(Some(credential.access_token))
    }
}
