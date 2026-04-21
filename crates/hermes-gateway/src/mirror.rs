//! Message mirroring with persistence.
//!
//! Routes messages from source channels to destination channels. Supports
//! bidirectional mirroring and persists routes to JSON.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

/// A mirror route.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorRoute {
    pub source: String,
    pub target: String,
    /// Whether to mirror in both directions.
    #[serde(default)]
    pub bidirectional: bool,
    /// Whether this route is currently active.
    #[serde(default = "default_true")]
    pub active: bool,
}

fn default_true() -> bool {
    true
}

/// Stores mirror routes between source and destination channels.
#[derive(Clone)]
pub struct MirrorManager {
    routes: Arc<RwLock<HashMap<String, MirrorRoute>>>,
    persist_path: Option<PathBuf>,
}

impl Default for MirrorManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MirrorManager {
    pub fn new() -> Self {
        Self {
            routes: Arc::new(RwLock::new(HashMap::new())),
            persist_path: None,
        }
    }

    /// Create with JSON persistence.
    pub fn with_persistence(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let routes = if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str::<HashMap<String, MirrorRoute>>(&s).ok())
                .unwrap_or_default()
        } else {
            HashMap::new()
        };
        Self {
            routes: Arc::new(RwLock::new(routes)),
            persist_path: Some(path),
        }
    }

    /// Set a mirror route from source to target.
    pub fn set_route(&self, source_channel: impl Into<String>, target_channel: impl Into<String>) {
        let source = source_channel.into();
        let target = target_channel.into();
        if let Ok(mut routes) = self.routes.write() {
            routes.insert(
                source.clone(),
                MirrorRoute {
                    source,
                    target,
                    bidirectional: false,
                    active: true,
                },
            );
        }
        self.persist();
    }

    /// Set a bidirectional mirror route.
    pub fn set_bidirectional_route(
        &self,
        channel_a: impl Into<String>,
        channel_b: impl Into<String>,
    ) {
        let a = channel_a.into();
        let b = channel_b.into();
        if let Ok(mut routes) = self.routes.write() {
            routes.insert(
                a.clone(),
                MirrorRoute {
                    source: a.clone(),
                    target: b.clone(),
                    bidirectional: true,
                    active: true,
                },
            );
            routes.insert(
                b.clone(),
                MirrorRoute {
                    source: b,
                    target: a,
                    bidirectional: true,
                    active: true,
                },
            );
        }
        self.persist();
    }

    /// Remove a mirror route.
    pub fn remove_route(&self, source_channel: &str) {
        if let Ok(mut routes) = self.routes.write() {
            if let Some(route) = routes.remove(source_channel) {
                // If bidirectional, also remove the reverse route
                if route.bidirectional {
                    routes.remove(&route.target);
                }
            }
        }
        self.persist();
    }

    /// Get the target channel for a source channel.
    pub fn route_for(&self, source_channel: &str) -> Option<String> {
        self.routes.read().ok().and_then(|r| {
            r.get(source_channel)
                .filter(|route| route.active)
                .map(|route| route.target.clone())
        })
    }

    /// List all active routes.
    pub fn list_routes(&self) -> Vec<MirrorRoute> {
        self.routes
            .read()
            .map(|r| r.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Pause a route without removing it.
    pub fn pause_route(&self, source_channel: &str) -> bool {
        let found = if let Ok(mut routes) = self.routes.write() {
            if let Some(route) = routes.get_mut(source_channel) {
                route.active = false;
                true
            } else {
                false
            }
        } else {
            false
        };
        if found {
            self.persist();
        }
        found
    }

    /// Resume a paused route.
    pub fn resume_route(&self, source_channel: &str) -> bool {
        let found = if let Ok(mut routes) = self.routes.write() {
            if let Some(route) = routes.get_mut(source_channel) {
                route.active = true;
                true
            } else {
                false
            }
        } else {
            false
        };
        if found {
            self.persist();
        }
        found
    }

    fn persist(&self) {
        if let Some(ref path) = self.persist_path {
            if let Ok(routes) = self.routes.read() {
                if let Ok(json) = serde_json::to_string_pretty(&*routes) {
                    if let Some(parent) = path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let tmp = path.with_extension("json.tmp");
                    if std::fs::write(&tmp, json.as_bytes()).is_ok() {
                        let _ = std::fs::rename(&tmp, path);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_route() {
        let mgr = MirrorManager::new();
        mgr.set_route("telegram:1", "discord:2");
        assert_eq!(mgr.route_for("telegram:1").unwrap(), "discord:2");
        assert!(mgr.route_for("discord:2").is_none()); // not bidirectional
    }

    #[test]
    fn bidirectional_route() {
        let mgr = MirrorManager::new();
        mgr.set_bidirectional_route("telegram:1", "discord:2");
        assert_eq!(mgr.route_for("telegram:1").unwrap(), "discord:2");
        assert_eq!(mgr.route_for("discord:2").unwrap(), "telegram:1");
    }

    #[test]
    fn remove_route() {
        let mgr = MirrorManager::new();
        mgr.set_route("a", "b");
        mgr.remove_route("a");
        assert!(mgr.route_for("a").is_none());
    }

    #[test]
    fn remove_bidirectional() {
        let mgr = MirrorManager::new();
        mgr.set_bidirectional_route("a", "b");
        mgr.remove_route("a");
        assert!(mgr.route_for("a").is_none());
        assert!(mgr.route_for("b").is_none());
    }

    #[test]
    fn pause_and_resume() {
        let mgr = MirrorManager::new();
        mgr.set_route("a", "b");
        assert!(mgr.pause_route("a"));
        assert!(mgr.route_for("a").is_none()); // paused
        assert!(mgr.resume_route("a"));
        assert_eq!(mgr.route_for("a").unwrap(), "b");
    }

    #[test]
    fn persist_and_reload() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("mirrors.json");

        {
            let mgr = MirrorManager::with_persistence(&path);
            mgr.set_route("telegram:1", "discord:2");
            mgr.set_bidirectional_route("slack:3", "matrix:4");
        }

        let mgr2 = MirrorManager::with_persistence(&path);
        assert_eq!(mgr2.route_for("telegram:1").unwrap(), "discord:2");
        assert_eq!(mgr2.route_for("slack:3").unwrap(), "matrix:4");
        assert_eq!(mgr2.route_for("matrix:4").unwrap(), "slack:3");
    }

    #[test]
    fn list_routes() {
        let mgr = MirrorManager::new();
        mgr.set_route("a", "b");
        mgr.set_route("c", "d");
        assert_eq!(mgr.list_routes().len(), 2);
    }
}
