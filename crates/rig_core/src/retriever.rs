//! Retriever module - Document retrieval and similarity search

use async_trait::async_trait;
use thiserror::Error;
use std::collections::HashMap;

#[derive(Error, Debug)]
pub enum RetrievalError {
    #[error("Index error: {0}")]
    IndexError(String),
    #[error("Search error: {0}")]
    SearchError(String),
}

/// A retrieved document with similarity score
#[derive(Debug, Clone)]
pub struct RetrievedDocument {
    pub content: String,
    pub metadata: HashMap<String, String>,
    pub similarity: f32,
}

/// Trait for document retrievers
#[async_trait]
pub trait Retriever: Send + Sync {
    /// Index a document with its embedding
    async fn index(
        &self,
        embedding: Vec<f32>,
        content: String,
        metadata: HashMap<String, String>,
    ) -> Result<(), RetrievalError>;

    /// Retrieve the top-k most similar documents
    async fn retrieve(
        &self,
        query_embedding: &[f32],
        k: usize,
    ) -> Result<Vec<RetrievedDocument>, RetrievalError>;

    /// Get the number of indexed documents
    async fn count(&self) -> Result<usize, RetrievalError>;
}

/// Calculate cosine similarity between two vectors
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot_product / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 0.0001);
    }
}