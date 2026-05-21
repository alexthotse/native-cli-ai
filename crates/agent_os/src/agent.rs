//! Agent module - Core agent implementation

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use thiserror::Error;

use nca_llm::LlmClient;
use crate::memory::Memory;
use crate::tools::{Tool, ToolRegistry};
use crate::planner::Plan;

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("LLM error: {0}")]
    LlmError(String),
    #[error("Tool execution error: {0}")]
    ToolError(String),
    #[error("Planning error: {0}")]
    PlanningError(String),
    #[error("Memory error: {0}")]
    MemoryError(String),
    #[error("Max iterations reached")]
    MaxIterationsReached,
}

/// Agent state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentState {
    Idle,
    Thinking,
    Executing,
    Waiting,
    Completed,
    Failed,
}

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub role: String,
    pub goals: Vec<String>,
    pub max_iterations: usize,
    pub temperature: f32,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            name: "Assistant".to_string(),
            role: "General assistant".to_string(),
            goals: vec!["Help the user".to_string()],
            max_iterations: 50,
            temperature: 0.7,
        }
    }
}

/// Core Agent struct
pub struct Agent {
    pub id: Uuid,
    pub config: AgentConfig,
    pub state: AgentState,
    pub llm_client: Box<dyn LlmClient>,
    pub memory: Memory,
    pub tool_registry: ToolRegistry,
    pub current_plan: Option<Plan>,
    pub iteration_count: usize,
}

impl Agent {
    pub fn new(config: AgentConfig, llm_client: Box<dyn LlmClient>) -> Self {
        Self {
            id: Uuid::new_v4(),
            config,
            state: AgentState::Idle,
            llm_client,
            memory: Memory::new(),
            tool_registry: ToolRegistry::new(),
            current_plan: None,
            iteration_count: 0,
        }
    }

    /// Register a tool with the agent
    pub fn register_tool(&mut self, tool: Box<dyn Tool>) {
        self.tool_registry.register(tool);
    }

    /// Execute a task
    pub async fn execute(&mut self, task: &str) -> Result<String, AgentError> {
        self.state = AgentState::Thinking;
        self.iteration_count = 0;

        // Create initial plan
        let plan = self.create_plan(task).await?;
        self.current_plan = Some(plan);

        // Execute plan steps
        while self.iteration_count < self.config.max_iterations {
            self.iteration_count += 1;
            
            match self.execute_next_step().await? {
                ExecutionResult::Completed(result) => {
                    self.state = AgentState::Completed;
                    return Ok(result);
                }
                ExecutionResult::Continue => continue,
                ExecutionResult::NeedMoreInfo => {
                    self.state = AgentState::Waiting;
                    return Err(AgentError::PlanningError("Need more information".to_string()));
                }
            }
        }

        self.state = AgentState::Failed;
        Err(AgentError::MaxIterationsReached)
    }

    async fn create_plan(&mut self, task: &str) -> Result<Plan, AgentError> {
        // Use LLM to create a plan
        let prompt = format!(
            "You are {}. Your role: {}. \nTask: {}\n\nCreate a step-by-step plan to accomplish this task.",
            self.config.name,
            self.config.role,
            task
        );

        let response = self.llm_client
            .chat(&prompt, None)
            .await
            .map_err(|e| AgentError::LlmError(e.to_string()))?;

        Plan::from_response(&response).map_err(|e| AgentError::PlanningError(e.to_string()))
    }

    async fn execute_next_step(&mut self) -> Result<ExecutionResult, AgentError> {
        // Get next step from current plan
        let plan = self.current_plan.as_ref()
            .ok_or_else(|| AgentError::PlanningError("No active plan".to_string()))?;

        let next_step = plan.get_next_step()
            .ok_or_else(|| AgentError::PlanningError("No more steps".to_string()))?;

        self.state = AgentState::Executing;

        // Check if step requires a tool
        if let Some(tool_name) = &next_step.tool {
            let result = self.tool_registry
                .execute(tool_name, &next_step.arguments)
                .await
                .map_err(|e| AgentError::ToolError(e.to_string()))?;
            
            self.memory.add_observation(&result);
        } else {
            // Use LLM for reasoning steps
            let response = self.llm_client
                .chat(&next_step.description, None)
                .await
                .map_err(|e| AgentError::LlmError(e.to_string()))?;
            
            self.memory.add_observation(&response);
        }

        plan.mark_step_completed(&next_step.id);

        if plan.is_complete() {
            Ok(ExecutionResult::Completed(self.memory.get_summary()))
        } else {
            Ok(ExecutionResult::Continue)
        }
    }

    /// Reflect on past actions to improve future performance
    pub async fn reflect(&mut self) -> Result<(), AgentError> {
        let history = self.memory.get_history();
        
        let prompt = format!(
            "Reflect on these past actions and identify improvements:\n{}",
            history
        );

        let reflection = self.llm_client
            .chat(&prompt, None)
            .await
            .map_err(|e| AgentError::LlmError(e.to_string()))?;

        self.memory.add_reflection(&reflection);
        Ok(())
    }
}

enum ExecutionResult {
    Completed(String),
    Continue,
    NeedMoreInfo,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_creation() {
        let config = AgentConfig::default();
        // Note: Would need a mock LLM client for full testing
        assert_eq!(config.name, "Assistant");
        assert_eq!(config.max_iterations, 50);
    }
}
