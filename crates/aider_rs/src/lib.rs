//! Aider Rust - AI-powered coding assistant
//! 
//! A Rust reimplementation of Aider (https://github.com/Aider-AI/aider)
//! Designed to be a Claude Code killer with:
//! - Fast, native performance
//! - Multi-provider LLM support
//! - Intelligent code editing with tree-sitter
//! - Git integration
//! - Agent-based architecture

pub mod editor;
pub mod git_integration;
pub mod codebase;
pub mod commands;
pub mod cli;

pub use editor::CodeEditor;
pub use git_integration::GitRepo;
pub use codebase::Codebase;
pub use commands::Command;

/// Aider configuration
#[derive(Debug, Clone)]
pub struct AiderConfig {
    pub auto_commit: bool,
    pub dry_run: bool,
    pub verbose: bool,
    pub max_edits_per_turn: usize,
}

impl Default for AiderConfig {
    fn default() -> Self {
        Self {
            auto_commit: true,
            dry_run: false,
            verbose: false,
            max_edits_per_turn: 10,
        }
    }
}

/// Error types for Aider operations
#[derive(Debug, thiserror::Error)]
pub enum AiderError {
    #[error("Editor error: {0}")]
    EditorError(String),
    #[error("Git error: {0}")]
    GitError(String),
    #[error("LLM error: {0}")]
    LlmError(String),
    #[error("IO error: {0}")]
    IoError(String),
}
