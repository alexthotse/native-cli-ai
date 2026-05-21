//! Document indexer with chunking and embedding

use crate::store::Document;
use std::collections::HashMap;

/// Indexer configuration
#[derive(Debug, Clone)]
pub struct IndexerConfig {
    /// Chunk size in tokens/characters
    pub chunk_size: usize,
    /// Overlap between chunks
    pub chunk_overlap: usize,
    /// Minimum chunk size (discard smaller chunks)
    pub min_chunk_size: usize,
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            chunk_size: 512,
            chunk_overlap: 50,
            min_chunk_size: 50,
        }
    }
}

/// Document indexer
pub struct Indexer {
    config: IndexerConfig,
}

impl Indexer {
    /// Create a new indexer with default config
    pub fn new() -> Self {
        Self {
            config: IndexerConfig::default(),
        }
    }

    /// Create with custom config
    pub fn with_config(config: IndexerConfig) -> Self {
        Self { config }
    }

    /// Chunk a document into smaller pieces
    pub fn chunk(&self, content: &str) -> Vec<String> {
        let mut chunks = Vec::new();
        let chars: Vec<char> = content.chars().collect();
        
        if chars.len() <= self.config.chunk_size {
            if chars.len() >= self.config.min_chunk_size {
                chunks.push(content.to_string());
            }
            return chunks;
        }

        let mut start = 0;
        while start < chars.len() {
            let end = (start + self.config.chunk_size).min(chars.len());
            let chunk: String = chars[start..end].iter().collect();
            
            if chunk.len() >= self.config.min_chunk_size {
                chunks.push(chunk);
            }
            
            // Move start position with overlap
            start += self.config.chunk_size - self.config.chunk_overlap;
            
            // Prevent infinite loop if overlap >= chunk_size
            if self.config.chunk_overlap >= self.config.chunk_size {
                break;
            }
        }

        chunks
    }

    /// Index a document, returning multiple chunked documents
    pub fn index(&self, id: &str, content: &str, metadata: HashMap<String, serde_json::Value>) -> Vec<Document> {
        let chunks = self.chunk(content);
        
        chunks
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| {
                let doc_id = format!("{}-chunk-{}", id, i);
                let mut doc = Document::new(doc_id, chunk);
                
                // Add chunk metadata
                doc.metadata.insert("source_id".to_string(), serde_json::json!(id));
                doc.metadata.insert("chunk_index".to_string(), serde_json::json!(i));
                
                // Merge with provided metadata
                for (k, v) in &metadata {
                    doc.metadata.insert(k.clone(), v.clone());
                }
                
                doc
            })
            .collect()
    }

    /// Get configuration
    pub fn config(&self) -> &IndexerConfig {
        &self.config
    }
}

impl Default for Indexer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunking_no_split_needed() {
        let indexer = Indexer::new();
        let content = "Short text";
        
        let chunks = indexer.chunk(content);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "Short text");
    }

    #[test]
    fn test_chunking_with_overlap() {
        let config = IndexerConfig {
            chunk_size: 10,
            chunk_overlap: 3,
            min_chunk_size: 5,
        };
        let indexer = Indexer::with_config(config);
        
        let content = "0123456789ABCDEFGHIJ";
        let chunks = indexer.chunk(content);
        
        // Should have multiple chunks with overlap
        assert!(chunks.len() > 1);
        
        // First chunk should be "0123456789"
        assert_eq!(chunks[0], "0123456789");
    }

    #[test]
    fn test_indexing_creates_multiple_docs() {
        let indexer = Indexer::with_config(IndexerConfig {
            chunk_size: 10,
            chunk_overlap: 0,
            min_chunk_size: 5,
        });
        
        let content = "This is a longer text that will be split into multiple chunks for testing purposes";
        let metadata = HashMap::new();
        
        let docs = indexer.index("test-doc", content, metadata);
        
        assert!(docs.len() > 1);
        
        // Check metadata is preserved
        for doc in &docs {
            assert!(doc.metadata.contains_key("source_id"));
            assert!(doc.metadata.contains_key("chunk_index"));
        }
    }
}
