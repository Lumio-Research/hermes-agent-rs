//! Benchmark environment implementations.
//!
//! Each benchmark wraps a dataset and provides task loading, environment
//! setup/teardown, and verification. Heavy dataset loading (HuggingFace)
//! is delegated to Python subprocess when needed.

pub mod swe_bench;
pub mod terminal_bench;
pub mod yc_bench;
