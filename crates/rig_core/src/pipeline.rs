//! RAG Pipeline module - Orchestrates retrieval and generation

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use nca_llm::LlmClient;
use crate::{RigConfig, RigError};
use super::embedding::Embedder;
use super::retriever::Retriever;
use super::context::ContextBuilder;

/// Result from a RAG query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagResult {
    pub answer: String,
    pub sources: Vec<SourceDocument>,
    pub context_used: String,
}

/// A source document used in the response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDocument {
    pub content: String,
    pub metadata: std::collections::HashMap<String, String>,
    pub relevance_score: f32,
}

/// RAG Pipeline that combines embedding, retrieval, and generation
pub struct RagPipeline<E, R> 
where
    E: Embedder,
    R: Retriever,
{
    embedder: E,
    retriever: R,
    context_builder: ContextBuilder,
    config: RigConfig,
}

impl<E, R> RagPipeline<E, R>
where
    E: Embedder,
    R: Retriever,
{
    pub fn new(embedder: E, retriever: R, config: RigConfig) -> Self {
        Self {
            embedder,
            retriever,
            context_builder: ContextBuilder::new(config.max_context_length),
            config,
        }
    }

    /// Execute a RAG query
    pub async fn query(
        &self,
        query: &str,
        llm_client: &dyn LlmClient,
    ) -> Result<RagResult, RigError> {
        // Step 1: Embed the query
        let query_embedding = self.embedder
            .embed(query)
            .await
            .map_err(|e| RigError::EmbeddingError(e.to_string()))?;

        // Step 2: Retrieve relevant documents
        let documents = self.retriever
            .retrieve(&query_embedding, self.config.top_k)
            .await
            .map_err(|e| RigError::RetrievalError(e.to_string()))?;

        // Step 3: Filter by similarity threshold
        let filtered_docs: Vec<_> = documents
            .into_iter()
            .filter(|d| d.similarity >= self.config.similarity_threshold)
            .collect();

        // Step 4: Build context from retrieved documents
        let context = self.context_builder
            .build(&filtered_docs)
            .map_err(|e| RigError::ContextError(e.to_string()))?;

        // Step 5: Generate answer using LLM
        let prompt = format!(
            "Based on the following context, answer the question.\n\nContext:\n{}\n\nQuestion: {}\n\nAnswer:",
            context,
            query
        );

        let answer = llm_client
            .chat(&prompt, None)
            .await
            .map_err(|e| RigError::GenerationError(e.to_string()))?;

        // Step 6: Format results
        let sources = filtered_docs
            .iter()
            .map(|d| SourceDocument {
                content: d.content.clone(),
                metadata: d.metadata.clone(),
                relevance_score: d.similarity,
            })
            .collect();

        Ok(RagResult {
            answer,
            sources,
            context_used: context,
        })
    }

    /// Index a new document
    pub async fn index_document(
        &self,
        content: &str,
        metadata: std::collections::HashMap<String, String>,
    ) -> Result<(), RigError> {
        let embedding = self.embedder
            .embed(content)
            .await
            .map_err(|e| RigError::EmbeddingError(e.to_string()))?;

        self.retriever
            .index(embedding, content.to_string(), metadata)
            .await
            .map_err(|e| RigError::RetrievalError(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rag_result_creation() {
        let result = RagResult {
            answer: "Test answer".to_string(),
            sources: vec![],
            context_used: "Test context".to_string(),
        };
        
        assert_eq!(result.answer, "Test answer");
    }
}