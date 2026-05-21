//! Commands module - Available commands for Aider Rust

use thiserror::Error;

#[derive(Error, Debug)]
pub enum CommandError {
    #[error("Unknown command: {0}")]
    UnknownCommand(String),
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
}

/// Available commands
#[derive(Debug, Clone)]
pub enum Command {
    /// Add a file to the chat context
    Add(String),
    /// Drop a file from the chat context
    Drop(String),
    /// List files in context
    Ls,
    /// Clear conversation history
    Clear,
    /// Show help
    Help,
    /// Exit the program
    Quit,
    /// Toggle auto-commit
    AutoCommit(bool),
    /// Toggle dry-run mode
    DryRun(bool),
    /// Switch model
    Model(String),
    /// Run a shell command
    Shell(String),
    /// Undo last edit
    Undo,
    /// Redo last undone edit
    Redo,
}

impl Command {
    /// Parse a command from user input
    pub fn parse(input: &str) -> Result<Self, CommandError> {
        let parts: Vec<&str> = input.trim().split_whitespace().collect();
        
        if parts.is_empty() {
            return Err(CommandError::UnknownCommand("".to_string()));
        }

        match parts[0] {
            "/add" | "/a" => {
                if parts.len() < 2 {
                    return Err(CommandError::InvalidArguments(
                        "Usage: /add <file>".to_string(),
                    ));
                }
                Ok(Command::Add(parts[1..].join(" ")))
            }
            "/drop" | "/d" => {
                if parts.len() < 2 {
                    return Err(CommandError::InvalidArguments(
                        "Usage: /drop <file>".to_string(),
                    ));
                }
                Ok(Command::Drop(parts[1..].join(" ")))
            }
            "/ls" | "/list" => Ok(Command::Ls),
            "/clear" => Ok(Command::Clear),
            "/help" | "/?" => Ok(Command::Help),
            "/quit" | "/exit" | "/q" => Ok(Command::Quit),
            "/autocommit" => {
                let enable = parts.get(1).map_or(true, |&s| s == "on" || s == "true");
                Ok(Command::AutoCommit(enable))
            }
            "/dryrun" => {
                let enable = parts.get(1).map_or(true, |&s| s == "on" || s == "true");
                Ok(Command::DryRun(enable))
            }
            "/model" | "/m" => {
                if parts.len() < 2 {
                    return Err(CommandError::InvalidArguments(
                        "Usage: /model <model-name>".to_string(),
                    ));
                }
                Ok(Command::Model(parts[1..].join(" ")))
            }
            "/shell" | "/sh" => {
                if parts.len() < 2 {
                    return Err(CommandError::InvalidArguments(
                        "Usage: /shell <command>".to_string(),
                    ));
                }
                Ok(Command::Shell(parts[1..].join(" ")))
            }
            "/undo" => Ok(Command::Undo),
            "/redo" => Ok(Command::Redo),
            _ => Err(CommandError::UnknownCommand(parts[0].to_string())),
        }
    }

    /// Get command description
    pub fn description(&self) -> &'static str {
        match self {
            Command::Add(_) => "Add a file to the chat context",
            Command::Drop(_) => "Remove a file from the chat context",
            Command::Ls => "List files in context",
            Command::Clear => "Clear conversation history",
            Command::Help => "Show help",
            Command::Quit => "Exit the program",
            Command::AutoCommit(_) => "Toggle auto-commit mode",
            Command::DryRun(_) => "Toggle dry-run mode",
            Command::Model(_) => "Switch to a different model",
            Command::Shell(_) => "Run a shell command",
            Command::Undo => "Undo last edit",
            Command::Redo => "Redo last undone edit",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_parse_add() {
        let cmd = Command::parse("/add src/main.rs").unwrap();
        assert!(matches!(cmd, Command::Add(s) if s == "src/main.rs"));
    }

    #[test]
    fn test_command_parse_help() {
        let cmd = Command::parse("/help").unwrap();
        assert!(matches!(cmd, Command::Help));
    }

    #[test]
    fn test_command_parse_unknown() {
        let result = Command::parse("/unknown");
        assert!(result.is_err());
    }
}