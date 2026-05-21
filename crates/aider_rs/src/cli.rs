//! CLI module - Command-line interface for Aider Rust

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "aider-rs")]
#[command(author = "NCA Team")]
#[command(version = "0.1.0")]
#[command(about = "AI-powered coding assistant (Claude Code killer)", long_about = None)]
pub struct Cli {
    /// The task or message to process
    #[arg(index = 1)]
    pub message: Option<String>,

    /// LLM provider to use (default: from NCA_DEFAULT_PROVIDER)
    #[arg(short, long, default_value = "minimax")]
    pub provider: String,

    /// Model to use (default: from provider config)
    #[arg(short, long)]
    pub model: Option<String>,

    /// Dry run mode - show what would be done without making changes
    #[arg(long)]
    pub dry_run: bool,

    /// Enable verbose output
    #[arg(short, long)]
    pub verbose: bool,

    /// Auto-commit changes to git
    #[arg(long, default_value = "true")]
    pub auto_commit: bool,

    /// Working directory (default: current directory)
    #[arg(short, long)]
    pub dir: Option<String>,

    /// Maximum number of edits per turn
    #[arg(long, default_value = "10")]
    pub max_edits: usize,
}