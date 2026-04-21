//! Context references — index layer for compressed conversation history.
//!
//! When the context compressor removes or summarizes old messages, important
//! references (file paths, URLs, variable names, code snippets) may be lost.
//! This module extracts and indexes those references before compression, so
//! the agent can still "remember" them even after the original messages are gone.
//!
//! Architecture:
//! ```text
//!   Messages → [ReferenceExtractor] → ReferenceIndex
//!                                          ↓
//!   Compressed messages + ReferenceIndex → Agent can query references
//! ```

use std::collections::{HashMap, HashSet};

use regex::Regex;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Reference types
// ---------------------------------------------------------------------------

/// A single extracted reference.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Reference {
    /// The reference kind.
    pub kind: ReferenceKind,
    /// The reference value (file path, URL, variable name, etc.).
    pub value: String,
    /// The message index where this reference was first seen.
    pub first_seen: usize,
    /// The message index where this reference was last seen.
    pub last_seen: usize,
    /// How many times this reference appeared.
    pub count: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    FilePath,
    Url,
    CodeSymbol,
    EnvVar,
    Command,
    GitRef,
    IpAddress,
    Other,
}

// ---------------------------------------------------------------------------
// Reference index
// ---------------------------------------------------------------------------

/// Index of all references extracted from conversation history.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReferenceIndex {
    /// All references, keyed by value for dedup.
    references: HashMap<String, Reference>,
    /// Kind → set of values for quick lookup.
    by_kind: HashMap<String, HashSet<String>>,
}

impl ReferenceIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add or update a reference.
    pub fn add(&mut self, kind: ReferenceKind, value: impl Into<String>, message_idx: usize) {
        let value = value.into();
        let kind_key = format!("{:?}", kind).to_lowercase();

        let entry = self.references.entry(value.clone()).or_insert_with(|| Reference {
            kind,
            value: value.clone(),
            first_seen: message_idx,
            last_seen: message_idx,
            count: 0,
        });
        entry.last_seen = message_idx;
        entry.count += 1;

        self.by_kind
            .entry(kind_key)
            .or_default()
            .insert(value);
    }

    /// Get all references of a specific kind.
    pub fn get_by_kind(&self, kind: ReferenceKind) -> Vec<&Reference> {
        let kind_key = format!("{:?}", kind).to_lowercase();
        self.by_kind
            .get(&kind_key)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|v| self.references.get(v))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all references.
    pub fn all(&self) -> Vec<&Reference> {
        self.references.values().collect()
    }

    /// Get references that were seen in a specific message range.
    pub fn in_range(&self, start: usize, end: usize) -> Vec<&Reference> {
        self.references
            .values()
            .filter(|r| r.last_seen >= start && r.first_seen <= end)
            .collect()
    }

    /// Number of unique references.
    pub fn len(&self) -> usize {
        self.references.len()
    }

    pub fn is_empty(&self) -> bool {
        self.references.is_empty()
    }

    /// Format as a compact summary string for injection into compressed context.
    pub fn summary(&self) -> String {
        if self.references.is_empty() {
            return String::new();
        }

        let mut parts = Vec::new();

        let files = self.get_by_kind(ReferenceKind::FilePath);
        if !files.is_empty() {
            let file_list: Vec<&str> = files.iter().map(|r| r.value.as_str()).collect();
            parts.push(format!("Files mentioned: {}", file_list.join(", ")));
        }

        let urls = self.get_by_kind(ReferenceKind::Url);
        if !urls.is_empty() {
            let url_list: Vec<&str> = urls.iter().map(|r| r.value.as_str()).collect();
            parts.push(format!("URLs referenced: {}", url_list.join(", ")));
        }

        let symbols = self.get_by_kind(ReferenceKind::CodeSymbol);
        if !symbols.is_empty() {
            let sym_list: Vec<&str> = symbols.iter().map(|r| r.value.as_str()).collect();
            parts.push(format!("Code symbols: {}", sym_list.join(", ")));
        }

        let env_vars = self.get_by_kind(ReferenceKind::EnvVar);
        if !env_vars.is_empty() {
            let env_list: Vec<&str> = env_vars.iter().map(|r| r.value.as_str()).collect();
            parts.push(format!("Env vars: {}", env_list.join(", ")));
        }

        let commands = self.get_by_kind(ReferenceKind::Command);
        if !commands.is_empty() {
            let cmd_list: Vec<&str> = commands.iter().take(10).map(|r| r.value.as_str()).collect();
            parts.push(format!("Commands used: {}", cmd_list.join(", ")));
        }

        parts.join("\n")
    }
}

// ---------------------------------------------------------------------------
// Reference extractor
// ---------------------------------------------------------------------------

/// Extracts references from message text.
pub struct ReferenceExtractor {
    file_path_re: Regex,
    url_re: Regex,
    env_var_re: Regex,
    #[allow(dead_code)]
    git_ref_re: Regex,
    ip_re: Regex,
}

impl ReferenceExtractor {
    pub fn new() -> Self {
        Self {
            file_path_re: Regex::new(
                r#"(?:^|[\s`"'(])(/[\w./-]{2,}|~/[\w./-]+|\.{1,2}/[\w./-]+)"#
            ).unwrap(),
            url_re: Regex::new(
                r#"https?://[^\s<>"')\]]+[^\s<>"')\].,;:!?]"#
            ).unwrap(),
            env_var_re: Regex::new(
                r"\$\{?([A-Z][A-Z0-9_]{2,})\}?"
            ).unwrap(),
            git_ref_re: Regex::new(
                r"\b([0-9a-f]{7,40})\b"
            ).unwrap(),
            ip_re: Regex::new(
                r"\b(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})\b"
            ).unwrap(),
        }
    }

    /// Extract all references from a text string.
    pub fn extract(&self, text: &str, message_idx: usize, index: &mut ReferenceIndex) {
        // File paths
        for cap in self.file_path_re.captures_iter(text) {
            if let Some(m) = cap.get(1) {
                let path = m.as_str().to_string();
                // Filter out common false positives
                if !is_common_false_positive(&path) {
                    index.add(ReferenceKind::FilePath, path, message_idx);
                }
            }
        }

        // URLs
        for m in self.url_re.find_iter(text) {
            index.add(ReferenceKind::Url, m.as_str(), message_idx);
        }

        // Environment variables
        for cap in self.env_var_re.captures_iter(text) {
            if let Some(m) = cap.get(1) {
                index.add(ReferenceKind::EnvVar, m.as_str(), message_idx);
            }
        }

        // IP addresses
        for cap in self.ip_re.captures_iter(text) {
            if let Some(m) = cap.get(1) {
                let ip = m.as_str();
                // Validate IP ranges
                let parts: Vec<u8> = ip
                    .split('.')
                    .filter_map(|p| p.parse().ok())
                    .collect();
                if parts.len() == 4 {
                    index.add(ReferenceKind::IpAddress, ip, message_idx);
                }
            }
        }
    }

    /// Extract references from a sequence of messages.
    pub fn extract_from_messages(
        &self,
        messages: &[impl AsRef<str>],
    ) -> ReferenceIndex {
        let mut index = ReferenceIndex::new();
        for (i, msg) in messages.iter().enumerate() {
            self.extract(msg.as_ref(), i, &mut index);
        }
        index
    }
}

impl Default for ReferenceExtractor {
    fn default() -> Self {
        Self::new()
    }
}

/// Filter out common false positives for file paths.
fn is_common_false_positive(path: &str) -> bool {
    // Version numbers like 1.0.0
    if path.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return true;
    }
    // Very short paths
    if path.len() < 4 {
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_file_paths() {
        let extractor = ReferenceExtractor::new();
        let mut index = ReferenceIndex::new();
        extractor.extract("Check the file /etc/hosts and ./config.yaml", 0, &mut index);

        let files = index.get_by_kind(ReferenceKind::FilePath);
        assert!(files.iter().any(|r| r.value == "/etc/hosts"));
        assert!(files.iter().any(|r| r.value == "./config.yaml"));
    }

    #[test]
    fn extract_urls() {
        let extractor = ReferenceExtractor::new();
        let mut index = ReferenceIndex::new();
        extractor.extract("Visit https://example.com/page and http://localhost:8080", 0, &mut index);

        let urls = index.get_by_kind(ReferenceKind::Url);
        assert_eq!(urls.len(), 2);
    }

    #[test]
    fn extract_env_vars() {
        let extractor = ReferenceExtractor::new();
        let mut index = ReferenceIndex::new();
        extractor.extract("Set $OPENAI_API_KEY and ${HOME}", 0, &mut index);

        let env_vars = index.get_by_kind(ReferenceKind::EnvVar);
        assert!(env_vars.iter().any(|r| r.value == "OPENAI_API_KEY"));
        assert!(env_vars.iter().any(|r| r.value == "HOME"));
    }

    #[test]
    fn extract_ip_addresses() {
        let extractor = ReferenceExtractor::new();
        let mut index = ReferenceIndex::new();
        extractor.extract("Connect to 192.168.1.1 or 10.0.0.1", 0, &mut index);

        let ips = index.get_by_kind(ReferenceKind::IpAddress);
        assert_eq!(ips.len(), 2);
    }

    #[test]
    fn reference_counting() {
        let extractor = ReferenceExtractor::new();
        let mut index = ReferenceIndex::new();
        extractor.extract("Edit /etc/hosts", 0, &mut index);
        extractor.extract("Check /etc/hosts again", 1, &mut index);

        let files = index.get_by_kind(ReferenceKind::FilePath);
        let hosts = files.iter().find(|r| r.value == "/etc/hosts").unwrap();
        assert_eq!(hosts.count, 2);
        assert_eq!(hosts.first_seen, 0);
        assert_eq!(hosts.last_seen, 1);
    }

    #[test]
    fn index_summary() {
        let extractor = ReferenceExtractor::new();
        let mut index = ReferenceIndex::new();
        extractor.extract("Edit /etc/hosts and visit https://example.com", 0, &mut index);

        let summary = index.summary();
        assert!(summary.contains("/etc/hosts"));
        assert!(summary.contains("https://example.com"));
    }

    #[test]
    fn index_in_range() {
        let extractor = ReferenceExtractor::new();
        let mut index = ReferenceIndex::new();
        extractor.extract("File /a.txt", 0, &mut index);
        extractor.extract("File /b.txt", 5, &mut index);
        extractor.extract("File /c.txt", 10, &mut index);

        let range = index.in_range(3, 7);
        assert_eq!(range.len(), 1);
        assert_eq!(range[0].value, "/b.txt");
    }

    #[test]
    fn extract_from_messages() {
        let extractor = ReferenceExtractor::new();
        let messages = vec![
            "Check /etc/hosts",
            "Visit https://example.com",
            "Set $API_KEY",
        ];
        let index = extractor.extract_from_messages(&messages);
        assert!(index.len() >= 3);
    }

    #[test]
    fn empty_text() {
        let extractor = ReferenceExtractor::new();
        let mut index = ReferenceIndex::new();
        extractor.extract("", 0, &mut index);
        assert!(index.is_empty());
    }

    #[test]
    fn serialization_roundtrip() {
        let mut index = ReferenceIndex::new();
        index.add(ReferenceKind::FilePath, "/test.txt", 0);
        index.add(ReferenceKind::Url, "https://example.com", 1);

        let json = serde_json::to_string(&index).unwrap();
        let parsed: ReferenceIndex = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 2);
    }
}
