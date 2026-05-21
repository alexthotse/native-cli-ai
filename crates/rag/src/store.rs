//! Vector store abstraction and document representation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Document with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Unique identifier
    pub id: String,
    /// Document content
    pub content: String,
    /// Embedding vector
    pub embedding: Vec<f32>,
    /// Metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Document {
    /// Create a new document
    pub fn new(id: String, content: String) -> Self {
        Self {
            id,
            content,
            embedding: vec![],
            metadata: HashMap::new(),
        }
    }

    /// Set the embedding vector
    pub fn with_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.embedding = embedding;
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: &str, value: serde_json::Value) -> Self {
        self.metadata.insert(key.to_string(), value);
        self
    }
}

/// Search result with score
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The matched document
    pub document: Document,
    /// Relevance score (higher is better)
    pub score: f32,
}

/// Vector store trait - can be implemented for different backends
pub trait VectorStore: Send + Sync {
    /// Add a document to the store
    fn add(&mut self, document: Document) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Add multiple documents
    fn add_batch(
        &mut self,
        documents: Vec<Document>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Search for similar documents
    fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>>;

    /// Delete a document by ID
    fn delete(&mut self, id: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>>;

    /// Get total document count
    fn count(&self) -> usize;

    /// Clear all documents
    fn clear(&mut self);
}

/// In-memory vector store implementation (for testing and small datasets)
#[derive(Debug, Default)]
pub struct InMemoryStore {
    documents: Vec<Document>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self {
            documents: Vec::new(),
        }
    }
}

impl VectorStore for InMemoryStore {
    fn add(&mut self, document: Document) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.documents.push(document);
        Ok(())
    }

    fn add_batch(
        &mut self,
        documents: Vec<Document>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.documents.extend(documents);
        Ok(())
    }

    fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        // Compute cosine similarity for each document
        let mut results: Vec<SearchResult> = self
            .documents
            .iter()
            .filter(|d| !d.embedding.is_empty())
            .map(|doc| {
                let score = cosine_similarity(query_embedding, &doc.embedding);
                SearchResult {
                    document: doc.clone(),
                    score,
                }
            })
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        // Take top k
        results.truncate(top_k);

        Ok(results)
    }

    fn delete(&mut self, id: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let initial_len = self.documents.len();
        self.documents.retain(|d| d.id != id);
        Ok(self.documents.len() < initial_len)
    }

    fn count(&self) -> usize {
        self.documents.len()
    }

    fn clear(&mut self) {
        self.documents.clear();
    }
}

/// Compute cosine similarity between two vectors
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
    fn test_cosine_similarity() {
        // Identical vectors should have similarity 1.0
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        // Orthogonal vectors should have similarity 0.0
        let c = vec![1.0, 0.0, 0.0];
        let d = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&c, &d).abs() < 0.001);

        // Opposite vectors should have similarity -1.0
        let e = vec![1.0, 2.0, 3.0];
        let f = vec![-1.0, -2.0, -3.0];
        assert!((cosine_similarity(&e, &f) + 1.0).abs() < 0.001);
    }

    #[test]
    fn test_in_memory_store() {
        let mut store = InMemoryStore::new();

        let doc1 = Document::new("1".to_string(), "First document".to_string())
            .with_embedding(vec![1.0, 0.0, 0.0]);
        let doc2 = Document::new("2".to_string(), "Second document".to_string())
            .with_embedding(vec![0.0, 1.0, 0.0]);

        store.add(doc1).unwrap();
        store.add(doc2).unwrap();

        assert_eq!(store.count(), 2);

        // Search with query similar to doc1
        let results = store.search(&[1.0, 0.0, 0.0], 1).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].document.id, "1");
    }
}
