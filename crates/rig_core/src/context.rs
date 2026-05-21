//! Context builder module - Builds context from retrieved documents

use thiserror::Error;
use super::retriever::RetrievedDocument;

#[derive(Error, Debug)]
pub enum ContextError {
    #[error("Context too long: {0}")]
    TooLong(String),
    #[error("No documents provided")]
    NoDocuments,
}

/// Builder for constructing context from retrieved documents
pub struct ContextBuilder {
    max_length: usize,
    separator: String,
    include_metadata: bool,
}

impl ContextBuilder {
    pub fn new(max_length: usize) -> Self {
        Self {
            max_length,
            separator: "\n\n---\n\n".to_string(),
            include_metadata: true,
        }
    }

    pub fn with_separator(mut self, separator: &str) -> Self {
        self.separator = separator.to_string();
        self
    }

    pub fn with_metadata(mut self, include: bool) -> Self {
        self.include_metadata = include;
        self
    }

    /// Build context from retrieved documents
    pub fn build(&self, documents: &[RetrievedDocument]) -> Result<String, ContextError> {
        if documents.is_empty() {
            return Err(ContextError::NoDocuments);
        }

        let mut context_parts = Vec::new();
        let mut current_length = 0;

        for doc in documents {
            let mut part = String::new();

            // Add metadata if enabled
            if self.include_metadata && !doc.metadata.is_empty() {
                let meta_str = doc
                    .metadata
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect::<Vec<_>>()
                    .join(", ");
                part.push_str(&format!("[{}] ", meta_str));
            }

            // Add content
            part.push_str(&doc.content);

            // Check if adding this would exceed max length
            if current_length + part.len() > self.max_length {
                // Truncate if necessary
                let remaining = self.max_length - current_length;
                if remaining > 10 {
                    part.truncate(remaining - 10);
                    part.push_str("...");
                    context_parts.push(part);
                }
                break;
            }

            context_parts.push(part);
            current_length += part.len() + self.separator.len();
        }

        if context_parts.is_empty() {
            return Err(ContextError::TooLong("All documents exceed max length".to_string()));
        }

        Ok(context_parts.join(&self.separator))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_context_builder_basic() {
        let builder = ContextBuilder::new(1000);
        
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), "test.txt".to_string());
        
        let doc = RetrievedDocument {
            content: "This is test content.".to_string(),
            metadata,
            similarity: 0.9,
        };

        let context = builder.build(&[doc]).unwrap();
        assert!(context.contains("source: test.txt"));
        assert!(context.contains("This is test content."));
    }

    #[test]
    fn test_context_builder_no_documents() {
        let builder = ContextBuilder::new(1000);
        let result = builder.build(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_context_builder_truncation() {
        let builder = ContextBuilder::new(50);
        
        let doc = RetrievedDocument {
            content: "This is a very long content that should be truncated".to_string(),
            metadata: HashMap::new(),
            similarity: 0.9,
        };

        let context = builder.build(&[doc]).unwrap();
        assert!(context.len() <= 50);
        assert!(context.ends_with("..."));
    }
}