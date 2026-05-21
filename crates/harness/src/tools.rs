//! Built-in and dynamic tools for the harness

use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ToolError {
    #[error("Tool execution failed: {0}")]
    ExecutionFailed(String),
    
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),
    
    #[error("Tool not found: {0}")]
    NotFound(String),
}

/// Tool result
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub metadata: HashMap<String, String>,
}

/// Built-in tool definitions
pub mod builtin {
    use super::*;
    
    /// Read a file
    pub fn file_read(path: &str) -> Result<ToolResult, ToolError> {
        std::fs::read_to_string(path)
            .map(|content| ToolResult {
                success: true,
                output: content,
                metadata: [("path".to_string(), path.to_string())].into_iter().collect(),
            })
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))
    }
    
    /// Write to a file
    pub fn file_write(path: &str, content: &str) -> Result<ToolResult, ToolError> {
        std::fs::write(path, content)
            .map(|_| ToolResult {
                success: true,
                output: format!("Written {} bytes to {}", content.len(), path),
                metadata: [("path".to_string(), path.to_string())].into_iter().collect(),
            })
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))
    }
    
    /// Execute shell command
    pub fn shell_exec(cmd: &str, args: &[&str]) -> Result<ToolResult, ToolError> {
        std::process::Command::new(cmd)
            .args(args)
            .output()
            .map(|output| ToolResult {
                success: output.status.success(),
                output: String::from_utf8_lossy(&output.stdout).to_string(),
                metadata: [
                    ("exit_code".to_string(), output.status.code().unwrap_or(-1).to_string()),
                    ("stderr".to_string(), String::from_utf8_lossy(&output.stderr).to_string()),
                ].into_iter().collect(),
            })
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))
    }
    
    /// List directory contents
    pub fn list_dir(path: &str) -> Result<ToolResult, ToolError> {
        std::fs::read_dir(path)
            .map(|entries| {
                let files: Vec<String> = entries
                    .filter_map(|e| e.ok().and_then(|e| e.file_name().into_string().ok()))
                    .collect();
                ToolResult {
                    success: true,
                    output: files.join("\n"),
                    metadata: [("path".to_string(), path.to_string())].into_iter().collect(),
                }
            })
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))
    }
}

/// Tool registry
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Fn(&[String]) -> Result<ToolResult, ToolError> + Send + Sync>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
        };
        
        // Register built-in tools
        registry.register_builtin_tools();
        
        registry
    }
    
    fn register_builtin_tools(&mut self) {
        self.tools.insert(
            "file_read".to_string(),
            Box::new(|args| {
                if args.is_empty() {
                    return Err(ToolError::InvalidParameters("Path required".to_string()));
                }
                builtin::file_read(&args[0])
            }),
        );
        
        self.tools.insert(
            "file_write".to_string(),
            Box::new(|args| {
                if args.len() < 2 {
                    return Err(ToolError::InvalidParameters("Path and content required".to_string()));
                }
                builtin::file_write(&args[0], &args[1])
            }),
        );
        
        self.tools.insert(
            "shell_exec".to_string(),
            Box::new(|args| {
                if args.is_empty() {
                    return Err(ToolError::InvalidParameters("Command required".to_string()));
                }
                let cmd = &args[0];
                let cmd_args: Vec<&str> = args[1..].iter().map(|s| s.as_str()).collect();
                builtin::shell_exec(cmd, &cmd_args)
            }),
        );
        
        self.tools.insert(
            "list_dir".to_string(),
            Box::new(|args| {
                let path = args.first().map(|s| s.as_str()).unwrap_or(".");
                builtin::list_dir(path)
            }),
        );
    }
    
    pub fn register_tool<F>(&mut self, name: String, func: F)
    where
        F: Fn(&[String]) -> Result<ToolResult, ToolError> + Send + Sync + 'static,
    {
        self.tools.insert(name, Box::new(func));
    }
    
    pub fn execute(&self, name: &str, args: &[String]) -> Result<ToolResult, ToolError> {
        self.tools
            .get(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?
            .call(args)
    }
    
    pub fn list_tools(&self) -> Vec<&String> {
        self.tools.keys().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_registry_creation() {
        let registry = ToolRegistry::new();
        assert!(registry.list_tools().len() >= 4); // At least 4 builtin tools
    }
    
    #[test]
    fn test_file_read_tool() {
        let registry = ToolRegistry::new();
        
        // Create a temp file
        let temp_path = "/tmp/test_tool.txt";
        std::fs::write(temp_path, "test content").unwrap();
        
        let result = registry.execute("file_read", &[temp_path.to_string()]).unwrap();
        assert!(result.success);
        assert_eq!(result.output, "test content");
        
        // Cleanup
        std::fs::remove_file(temp_path).ok();
    }
    
    #[test]
    fn test_list_dir_tool() {
        let registry = ToolRegistry::new();
        let result = registry.execute("list_dir", &[".".to_string()]).unwrap();
        assert!(result.success);
        assert!(!result.output.is_empty());
    }
}
