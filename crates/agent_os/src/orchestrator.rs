//! Orchestrator module - Multi-agent orchestration and coordination

use std::collections::HashMap;
use uuid::Uuid;
use thiserror::Error;

use crate::agent::{Agent, AgentConfig, AgentState};
use crate::events::{Event, EventBus, EventType};
use crate::AgentOsConfig;

#[derive(Error, Debug)]
pub enum OrchestratorError {
    #[error("Agent not found: {0}")]
    AgentNotFound(String),
    #[error("Agent already exists: {0}")]
    AgentAlreadyExists(String),
    #[error("Execution error: {0}")]
    ExecutionError(String),
    #[error("Communication error: {0}")]
    CommunicationError(String),
}

/// Role-based agent assignment
#[derive(Debug, Clone)]
pub struct RoleAssignment {
    pub role: String,
    pub agent_id: String,
    pub responsibilities: Vec<String>,
}

/// Multi-agent orchestrator
pub struct Orchestrator {
    pub id: String,
    pub agents: HashMap<String, Agent>,
    pub role_assignments: HashMap<String, RoleAssignment>,
    pub event_bus: EventBus,
    pub config: AgentOsConfig,
    pub active_tasks: Vec<String>,
}

impl Orchestrator {
    pub fn new(config: AgentOsConfig) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            agents: HashMap::new(),
            role_assignments: HashMap::new(),
            event_bus: EventBus::new(),
            config,
            active_tasks: Vec::new(),
        }
    }

    /// Register an agent with the orchestrator
    pub fn register_agent(&mut self, agent: Agent) -> Result<(), OrchestratorError> {
        let agent_id = agent.id.to_string();
        
        if self.agents.contains_key(&agent_id) {
            return Err(OrchestratorError::AgentAlreadyExists(agent_id));
        }

        self.event_bus.publish(&Event::info(
            &self.id,
            &format!("Registering agent {}", agent_id),
        ));

        self.agents.insert(agent_id.clone(), agent);
        Ok(())
    }

    /// Remove an agent from the orchestrator
    pub fn remove_agent(&mut self, agent_id: &str) -> Result<(), OrchestratorError> {
        if !self.agents.contains_key(agent_id) {
            return Err(OrchestratorError::AgentNotFound(agent_id.to_string()));
        }

        self.agents.remove(agent_id);
        self.event_bus.publish(&Event::info(
            &self.id,
            &format!("Removed agent {}", agent_id),
        ));

        Ok(())
    }

    /// Assign a role to an agent
    pub fn assign_role(
        &mut self,
        agent_id: &str,
        role: &str,
        responsibilities: Vec<String>,
    ) -> Result<(), OrchestratorError> {
        if !self.agents.contains_key(agent_id) {
            return Err(OrchestratorError::AgentNotFound(agent_id.to_string()));
        }

        let assignment = RoleAssignment {
            role: role.to_string(),
            agent_id: agent_id.to_string(),
            responsibilities,
        };

        self.role_assignments.insert(role.to_string(), assignment);
        
        self.event_bus.publish(&Event::info(
            &self.id,
            &format!("Assigned role {} to agent {}", role, agent_id),
        ));

        Ok(())
    }

    /// Execute a task with a specific agent
    pub async fn execute_task(
        &mut self,
        agent_id: &str,
        task: &str,
    ) -> Result<String, OrchestratorError> {
        let agent = self.agents.get_mut(agent_id)
            .ok_or_else(|| OrchestratorError::AgentNotFound(agent_id.to_string()))?;

        self.active_tasks.push(task.to_string());
        
        self.event_bus.publish(&Event::agent_started(agent_id, task));

        match agent.execute(task).await {
            Ok(result) => {
                self.active_tasks.retain(|t| t != task);
                self.event_bus.publish(&Event::info(
                    agent_id,
                    &format!("Task completed: {}", task),
                ));
                Ok(result)
            }
            Err(e) => {
                self.active_tasks.retain(|t| t != task);
                self.event_bus.publish(&Event::error(
                    agent_id,
                    &format!("Task failed: {}", e),
                ));
                Err(OrchestratorError::ExecutionError(e.to_string()))
            }
        }
    }

    /// Execute a task with multi-agent collaboration
    pub async fn execute_collaborative(
        &mut self,
        task: &str,
        agent_ids: Vec<&str>,
    ) -> Result<String, OrchestratorError> {
        if !self.config.multi_agent_enabled {
            return Err(OrchestratorError::ExecutionError(
                "Multi-agent mode is disabled".to_string(),
            ));
        }

        self.event_bus.publish(&Event::info(
            &self.id,
            &format!("Starting collaborative task with {} agents", agent_ids.len()),
        ));

        let mut results = Vec::new();
        
        // Execute task with each agent sequentially (can be parallelized)
        for agent_id in agent_ids {
            let result = self.execute_task(agent_id, task).await?;
            results.push((agent_id.to_string(), result));
        }

        // Aggregate results
        let summary = results
            .iter()
            .map(|(id, r)| format!("Agent {}: {}", id, r))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(summary)
    }

    /// Broadcast a message to all agents
    pub fn broadcast(&self, message: &str) {
        self.event_bus.publish(&Event::builder(
            EventType::Broadcast,
            &self.id,
        )
        .payload(serde_json::json!({"message": message}))
        .build());
    }

    /// Get agent by ID
    pub fn get_agent(&self, agent_id: &str) -> Option<&Agent> {
        self.agents.get(agent_id)
    }

    /// Get agent by ID (mutable)
    pub fn get_agent_mut(&mut self, agent_id: &str) -> Option<&mut Agent> {
        self.agents.get_mut(agent_id)
    }

    /// List all registered agents
    pub fn list_agents(&self) -> Vec<&str> {
        self.agents.keys().map(|s| s.as_str()).collect()
    }

    /// Get agents by role
    pub fn get_agents_by_role(&self, role: &str) -> Vec<&Agent> {
        self.role_assignments
            .get(role)
            .and_then(|assignment| self.agents.get(&assignment.agent_id))
            .into_iter()
            .collect()
    }

    /// Get orchestrator status
    pub fn status(&self) -> OrchestratorStatus {
        let mut agent_states = HashMap::new();
        
        for (id, agent) in &self.agents {
            agent_states.insert(id.clone(), format!("{:?}", agent.state));
        }

        OrchestratorStatus {
            id: self.id.clone(),
            total_agents: self.agents.len(),
            active_tasks: self.active_tasks.len(),
            agent_states,
            multi_agent_enabled: self.config.multi_agent_enabled,
        }
    }

    /// Enable reflection for all agents
    pub fn enable_reflection(&mut self) {
        for agent in self.agents.values_mut() {
            let _ = agent.reflect(); // Fire and forget
        }
    }
}

/// Orchestrator status information
#[derive(Debug, Clone)]
pub struct OrchestratorStatus {
    pub id: String,
    pub total_agents: usize,
    pub active_tasks: usize,
    pub agent_states: HashMap<String, String>,
    pub multi_agent_enabled: bool,
}

impl Default for Orchestrator {
    fn default() -> Self {
        Self::new(AgentOsConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orchestrator_creation() {
        let orchestrator = Orchestrator::new(AgentOsConfig::default());
        
        assert_eq!(orchestrator.agents.len(), 0);
        assert!(!orchestrator.config.multi_agent_enabled);
    }

    #[test]
    fn test_orchestrator_status() {
        let orchestrator = Orchestrator::new(AgentOsConfig::default());
        let status = orchestrator.status();
        
        assert_eq!(status.total_agents, 0);
        assert_eq!(status.active_tasks, 0);
    }
}
