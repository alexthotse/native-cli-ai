//! Self-healing execution engine

use std::sync::Arc;
use tokio::time::{Duration, sleep};
use crate::state::{HarnessState, TaskStatus};
use crate::sandbox::Sandbox;
use crate::swarm::AgentSwarm;
use crate::synthesizer::ToolSynthesizer;
use uuid::Uuid;

/// Maximum retry attempts before giving up
const MAX_RETRIES: u32 = 5;

/// Result of a healing cycle
#[derive(Debug)]
pub struct HealingResult {
    pub success: bool,
    pub attempts: u32,
    pub final_error: Option<String>,
    pub patches_applied: Vec<String>,
}

/// Core self-healing harness engine
pub struct HarnessEngine {
    pub state: Arc<HarnessState>,
    pub sandbox: Sandbox,
    pub swarm: AgentSwarm,
    pub synthesizer: ToolSynthesizer,
    pub max_retries: u32,
    pub timeout: Duration,
}

impl HarnessEngine {
    pub fn new() -> Self {
        let state = Arc::new(HarnessState::new());
        let swarm = AgentSwarm::new(state.clone());
        
        Self {
            state,
            sandbox: Sandbox::new(),
            swarm,
            synthesizer: ToolSynthesizer::new(),
            max_retries: MAX_RETRIES,
            timeout: Duration::from_secs(300), // 5 minutes
        }
    }
    
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
    
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }
    
    /// Execute a task with self-healing capabilities
    pub async fn execute_with_healing(&self, description: &str) -> HealingResult {
        let task_id = self.state.add_task(description.to_string());
        let mut attempts = 0;
        let mut patches = Vec::new();
        
        println!("🚀 Starting self-healing execution for: {}", description);
        
        loop {
            attempts += 1;
            self.state.increment_attempts(&task_id);
            
            println!("🔄 Attempt {} of {}", attempts, self.max_retries);
            
            // Phase 1: Generate/Update code with swarm
            match self.swarm.execute_task(&task_id, description).await {
                Ok(output) => {
                    println!("✅ Swarm execution successful");
                    self.state.add_history("engine".to_string(), output);
                }
                Err(e) => {
                    println!("❌ Swarm execution failed: {}", e);
                    self.state.add_error(e.clone());
                }
            }
            
            // Phase 2: Compile and check
            println!("🔨 Compiling...");
            match self.sandbox.check(".") {
                Ok(result) if result.success => {
                    println!("✅ Compilation successful");
                    self.state.clear_errors();
                    
                    // Phase 3: Run tests
                    println!("🧪 Running tests...");
                    match self.sandbox.test(".") {
                        Ok(test_result) if test_result.success => {
                            println!("✅ All tests passed!");
                            self.state.update_task_status(&task_id, TaskStatus::Completed);
                            
                            return HealingResult {
                                success: true,
                                attempts,
                                final_error: None,
                                patches_applied: patches,
                            };
                        }
                        Ok(test_result) => {
                            println!("❌ Tests failed:\n{}", test_result.stderr);
                            self.state.add_error(format!("Test failure: {}", test_result.stderr));
                        }
                        Err(e) => {
                            println!("❌ Test execution failed: {}", e);
                            self.state.add_error(format!("Test error: {}", e));
                        }
                    }
                }
                Ok(result) => {
                    println!("❌ Compilation failed:\n{}", result.stderr);
                    self.state.add_error(format!("Compilation error: {}", result.stderr));
                }
                Err(e) => {
                    println!("❌ Compilation check failed: {}", e);
                    self.state.add_error(format!("Check error: {}", e));
                }
            }
            
            // Check if we've exceeded max retries
            if attempts >= self.max_retries {
                println!("💥 Max retries ({}) exceeded. Giving up.", self.max_retries);
                self.state.update_task_status(&task_id, TaskStatus::Failed("Max retries exceeded".to_string()));
                
                return HealingResult {
                    success: false,
                    attempts,
                    final_error: Some(self.state.get_errors().join("\n")),
                    patches_applied: patches,
                };
            }
            
            // Phase 4: Analyze errors and generate fix
            println!("🔍 Analyzing errors for automatic fix...");
            let errors = self.state.get_errors();
            if errors.is_empty() {
                continue;
            }
            
            // Use debugger agent to analyze and fix
            let error_context = errors.join("\n");
            match self.swarm.get_active_agents().iter().find(|a| {
                matches!(a.role, crate::swarm::DeveloperRole::Debugger)
            }) {
                Some(debugger) => {
                    println!("👨‍⚕️ Debugger analyzing...");
                    match debugger.agent.process(&format!("Fix these errors: {}", error_context)).await {
                        Ok(fix) => {
                            println!("🩹 Applying fix...");
                            patches.push(fix.clone());
                            self.state.add_patch(fix);
                            self.state.clear_errors();
                            
                            // Small delay before retry
                            sleep(Duration::from_millis(500)).await;
                            continue;
                        }
                        Err(e) => {
                            println!("❌ Failed to generate fix: {}", e);
                            self.state.add_error(format!("Fix generation failed: {}", e));
                        }
                    }
                }
                None => {
                    println!("⚠️ No debugger agent available");
                }
            }
            
            // If we can't fix automatically, break
            break;
        }
        
        self.state.update_task_status(&task_id, TaskStatus::Failed("Unrecoverable errors".to_string()));
        
        HealingResult {
            success: false,
            attempts,
            final_error: Some(self.state.get_errors().join("\n")),
            patches_applied: patches,
        }
    }
    
    /// Get current state summary
    pub fn get_summary(&self) -> String {
        let tasks = self.state.tasks.len();
        let errors = self.state.get_errors().len();
        let patches = self.state.patches.read().len();
        let agents = self.swarm.get_active_agents().len();
        
        format!(
            "Harness Engine Summary:\n\
             ├─ Tasks: {}\n\
             ├─ Errors: {}\n\
             ├─ Patches: {}\n\
             └─ Active Agents: {}",
            tasks, errors, patches, agents
        )
    }
}

impl Default for HarnessEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_engine_creation() {
        let engine = HarnessEngine::new();
        assert_eq!(engine.max_retries, MAX_RETRIES);
        assert_eq!(engine.timeout, Duration::from_secs(300));
    }
    
    #[test]
    fn test_engine_config() {
        let engine = HarnessEngine::new()
            .with_timeout(Duration::from_secs(60))
            .with_max_retries(10);
        
        assert_eq!(engine.timeout, Duration::from_secs(60));
        assert_eq!(engine.max_retries, 10);
    }
    
    #[tokio::test]
    async fn test_get_summary() {
        let engine = HarnessEngine::new();
        let summary = engine.get_summary();
        assert!(summary.contains("Harness Engine Summary"));
        assert!(summary.contains("Tasks:"));
    }
}
