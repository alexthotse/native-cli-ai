//! Agent definition and lifecycle management

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Unique identifier for an agent
pub type AgentId = Uuid;

/// Agent lifecycle state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentState {
    /// Agent is being initialized
    Initializing,
    /// Agent is ready to accept tasks
    Idle,
    /// Agent is currently executing a task
    Busy,
    /// Agent is paused (state preserved)
    Paused,
    /// Agent has completed all tasks
    Completed,
    /// Agent encountered an error
    Error(String),
    /// Agent has been terminated
    Terminated,
}

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Human-readable name for the agent
    pub name: String,
    /// LLM provider to use (e.g., "ollamacloud", "claude", "openai")
    pub provider: String,
    /// Model to use with the provider
    pub model: String,
    /// List of tools this agent can use
    pub tools: Vec<String>,
    /// Guardrails for sensitive operations
    pub guardrails: Vec<String>,
    /// Optional schedule (cron expression)
    pub schedule: Option<String>,
    /// Additional configuration parameters
    pub extra: HashMap<String, serde_json::Value>,
}

/// Agent representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    /// Unique agent identifier
    pub id: AgentId,
    /// Current state of the agent
    pub state: AgentState,
    /// Agent configuration
    pub config: AgentConfig,
    /// Timestamp when agent was created
    pub created_at: DateTime<Utc>,
    /// Timestamp of last state change
    pub updated_at: DateTime<Utc>,
    /// Current task ID if busy
    pub current_task_id: Option<uuid::Uuid>,
    /// Total tasks completed
    pub tasks_completed: u64,
    /// Total tasks failed
    pub tasks_failed: u64,
}

impl Agent {
    /// Create a new agent with the given configuration
    pub fn new(config: AgentConfig) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            state: AgentState::Initializing,
            config,
            created_at: now,
            updated_at: now,
            current_task_id: None,
            tasks_completed: 0,
            tasks_failed: 0,
        }
    }

    /// Transition agent to a new state
    pub fn transition_to(&mut self, new_state: AgentState) {
        self.state = new_state;
        self.updated_at = Utc::now();
    }

    /// Check if agent is available for new tasks
    pub fn is_available(&self) -> bool {
        matches!(self.state, AgentState::Idle)
    }

    /// Assign a task to the agent
    pub fn assign_task(&mut self, task_id: uuid::Uuid) -> Result<(), &'static str> {
        if !self.is_available() {
            return Err("Agent is not available");
        }
        self.current_task_id = Some(task_id);
        self.transition_to(AgentState::Busy);
        Ok(())
    }

    /// Mark current task as completed
    pub fn complete_task(&mut self) {
        self.current_task_id = None;
        self.tasks_completed += 1;
        self.transition_to(AgentState::Idle);
    }

    /// Mark current task as failed
    pub fn fail_task(&mut self, error: String) {
        self.current_task_id = None;
        self.tasks_failed += 1;
        self.transition_to(AgentState::Error(error));
    }

    /// Pause the agent
    pub fn pause(&mut self) -> Result<(), &'static str> {
        if matches!(self.state, AgentState::Busy) {
            return Err("Cannot pause busy agent");
        }
        self.transition_to(AgentState::Paused);
        Ok(())
    }

    /// Resume a paused agent
    pub fn resume(&mut self) -> Result<(), &'static str> {
        if !matches!(self.state, AgentState::Paused) {
            return Err("Agent is not paused");
        }
        self.transition_to(AgentState::Idle);
        Ok(())
    }

    /// Terminate the agent
    pub fn terminate(&mut self) {
        self.transition_to(AgentState::Terminated);
        self.current_task_id = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> AgentConfig {
        AgentConfig {
            name: "test-agent".to_string(),
            provider: "openai".to_string(),
            model: "gpt-4".to_string(),
            tools: vec!["file_read".to_string(), "shell".to_string()],
            guardrails: vec![],
            schedule: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn test_agent_creation() {
        let config = test_config();
        let agent = Agent::new(config.clone());
        
        assert_eq!(agent.config.name, "test-agent");
        assert_eq!(agent.state, AgentState::Initializing);
        assert!(agent.current_task_id.is_none());
    }

    #[test]
    fn test_agent_state_transitions() {
        let config = test_config();
        let mut agent = Agent::new(config);
        
        // Initialize -> Idle
        agent.transition_to(AgentState::Idle);
        assert!(agent.is_available());
        
        // Idle -> Busy (via task assignment)
        let task_id = Uuid::new_v4();
        assert!(agent.assign_task(task_id).is_ok());
        assert!(!agent.is_available());
        assert_eq!(agent.current_task_id, Some(task_id));
        
        // Busy -> Idle (via task completion)
        agent.complete_task();
        assert!(agent.is_available());
        assert_eq!(agent.tasks_completed, 1);
    }

    #[test]
    fn test_agent_pause_resume() {
        let config = test_config();
        let mut agent = Agent::new(config);
        agent.transition_to(AgentState::Idle);
        
        assert!(agent.pause().is_ok());
        assert_eq!(agent.state, AgentState::Paused);
        
        assert!(agent.resume().is_ok());
        assert_eq!(agent.state, AgentState::Idle);
    }
}
