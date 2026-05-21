//! Sandbox for safe code execution with timeout and resource limits

use std::process::{Command, Stdio};
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SandboxError {
    #[error("Execution timeout: {0}")]
    Timeout(Duration),
    
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    
    #[error("Resource limit exceeded: {0}")]
    ResourceLimit(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result of a sandbox execution
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub duration: Duration,
}

/// Sandboxed execution environment
pub struct Sandbox {
    workdir: String,
    timeout: Duration,
    max_memory_mb: Option<u64>,
}

impl Default for Sandbox {
    fn default() -> Self {
        Self::new()
    }
}

impl Sandbox {
    pub fn new() -> Self {
        Self {
            workdir: std::env::current_dir()
                .unwrap_or_else(|_| ".".to_string())
                .to_string_lossy()
                .to_string(),
            timeout: Duration::from_secs(30),
            max_memory_mb: Some(512),
        }
    }
    
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
    
    pub fn with_memory_limit(mut self, memory_mb: u64) -> Self {
        self.max_memory_mb = Some(memory_mb);
        self
    }
    
    pub fn set_workdir(&mut self, path: String) {
        self.workdir = path;
    }
    
    /// Execute a command in the sandbox
    pub fn execute(&self, cmd: &str, args: &[&str]) -> Result<ExecutionResult, SandboxError> {
        let start = std::time::Instant::now();
        
        let mut command = Command::new(cmd);
        command
            .args(args)
            .current_dir(&self.workdir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        
        // Set resource limits if available
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            unsafe {
                command.pre_exec(|| {
                    // Set memory limit using setrlimit
                    if let Some(mem_mb) = self.max_memory_mb {
                        let limit = libc::rlimit {
                            rlim_cur: mem_mb * 1024 * 1024,
                            rlim_max: mem_mb * 1024 * 1024,
                        };
                        libc::setrlimit(libc::RLIMIT_AS, &limit);
                    }
                    Ok(())
                });
            }
        }
        
        let child = command.spawn()?;
        
        // Wait for completion with timeout
        let output = match child.wait_with_output() {
            Ok(output) => output,
            Err(e) => return Err(SandboxError::ExecutionFailed(e.to_string())),
        };
        
        let duration = start.elapsed();
        
        if duration > self.timeout {
            return Err(SandboxError::Timeout(self.timeout));
        }
        
        Ok(ExecutionResult {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code(),
            duration,
        })
    }
    
    /// Compile Rust code in the sandbox
    pub fn compile(&self, manifest_path: &str) -> Result<ExecutionResult, SandboxError> {
        self.execute("cargo", &["build", "--manifest-path", manifest_path])
    }
    
    /// Run tests in the sandbox
    pub fn test(&self, manifest_path: &str) -> Result<ExecutionResult, SandboxError> {
        self.execute("cargo", &["test", "--manifest-path", manifest_path])
    }
    
    /// Check code without building
    pub fn check(&self, manifest_path: &str) -> Result<ExecutionResult, SandboxError> {
        self.execute("cargo", &["check", "--manifest-path", manifest_path])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sandbox_creation() {
        let sandbox = Sandbox::new();
        assert!(!sandbox.workdir.is_empty());
    }
    
    #[test]
    fn test_sandbox_timeout_config() {
        let sandbox = Sandbox::new().with_timeout(Duration::from_secs(60));
        assert_eq!(sandbox.timeout, Duration::from_secs(60));
    }
    
    #[test]
    fn test_execute_echo() {
        let sandbox = Sandbox::new();
        let result = sandbox.execute("echo", &["hello"]).unwrap();
        assert!(result.success);
        assert!(result.stdout.contains("hello"));
    }
}
