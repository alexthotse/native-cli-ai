//! Embedding module - Document embedding traits and implementations

use async_trait::async_trait;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EmbeddingError {
    #[error("Failed to generate embedding: {0}")]
    GenerationError(String),
    #[error("Model error: {0}")]
    ModelError(String),
}

/// Trait for text embedding models
#[async_trait]
pub trait Embedder: Send + Sync {
    /// Generate embeddings for a single text
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;

    /// Generate embeddings for multiple texts (batch)
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let mut embeddings = Vec::new();
        for text in texts {
            embeddings.push(self.embed(text).await?);
        }
        Ok(embeddings)
    }

    /// Get the embedding dimension
    fn dimension(&self) -> usize;
}

/// A document with its embedding
#[derive(Debug, Clone)]
pub struct EmbeddedDocument {
    pub content: String,
    pub embedding: Vec<f32>,
    pub metadata: std::collections::HashMap<String, String>,
}

impl EmbeddedDocument {
    pub fn new(content: &str, embedding: Vec<f32>) -> Self {
        Self {
            content: content.to_string(),
            embedding,
            metadata: std::collections::HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, metadata: std::collections::HashMap<String, String>) -> Self {
        self.metadata = metadata;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_document_creation() {
        let doc = EmbeddedDocument::new("test content", vec![0.1, 0.2, 0.3]);
        
        assert_eq!(doc.content, "test content");
        assert_eq!(doc.embedding.len(), 3);
    }
}