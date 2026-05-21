//! Rig Core - Rust implementation of Rig RLM (Retrieval-Augmented Language Model)
//! 
//! Inspired by https://github.com/joshua-mo-143/rig-rlm
//! 
//! This crate provides:
//! - Retrieval-augmented generation (RAG) pipelines
//! - Document embedding and indexing
//! - Semantic search capabilities
//! - Integration with multiple LLM providers

pub mod pipeline;
pub mod embedding;
pub mod retriever;
pub mod context;

pub use pipeline::RagPipeline;
pub use embedding::Embedder;
pub use retriever::Retriever;
pub use context::ContextBuilder;

/// Rig configuration
#[derive(Debug, Clone)]
pub struct RigConfig {
    pub top_k: usize,
    pub similarity_threshold: f32,
    pub max_context_length: usize,
    pub include_sources: bool,
}

impl Default for RigConfig {
    fn default() -> Self {
        Self {
            top_k: 5,
            similarity_threshold: 0.7,
            max_context_length: 4096,
            include_sources: true,
        }
    }
}

/// Error types for Rig operations
#[derive(Debug, thiserror::Error)]
pub enum RigError {
    #[error("Embedding error: {0}")]
    EmbeddingError(String),
    #[error("Retrieval error: {0}")]
    RetrievalError(String),
    #[error("Generation error: {0}")]
    GenerationError(String),
    #[error("Context error: {0}")]
    ContextError(String),
}
