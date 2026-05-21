//! NCA RAG - Retrieval-Augmented Generation pipeline
//! 
//! Based on rig-rlm architecture, this crate provides:
//! - Document indexing and embedding
//! - Vector store abstraction
//! - Hybrid search (semantic + keyword)
//! - Context assembly for LLMs

pub mod store;
pub mod retriever;
pub mod indexer;
pub mod context;

pub use store::{VectorStore, Document};
pub use retriever::Retriever;
pub use indexer::Indexer;
pub use context::ContextBuilder;
