//! Multi-agent orchestrator

use crate::agent::{Agent, AgentConfig, AgentId, AgentState};
use crate::task::{Task, TaskId, TaskStatus};
use crate::events::{Event, EventBus};
use std::collections::HashMap;
use tokio::sync::RwLock;
use std::sync::Arc;

/// Orchestrator configuration
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Maximum number of concurrent agents
    pub max_concurrent_agents: usize,
    /// Default task timeout in seconds
    pub task_timeout_seconds: u64,
    /// Operations requiring approval
    pub approval_required_for: Vec<String>,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_agents: 5,
            task_timeout_seconds: 300,
            approval_required_for: vec![],
        }
    }
}

/// Multi-agent orchestrator
pub struct Orchestrator {
    config: OrchestratorConfig,
    agents: Arc<RwLock<HashMap<AgentId, Agent>>>,
    tasks: Arc<RwLock<HashMap<TaskId, Task>>>,
    event_bus: EventBus,
}

impl Orchestrator {
    /// Create a new orchestrator with the given configuration
    pub fn new(config: OrchestratorConfig) -> Self {
        Self {
            config,
            agents: Arc::new(RwLock::new(HashMap::new())),
            tasks: Arc::new(RwLock::new(HashMap::new())),
            event_bus: EventBus::new(),
        }
    }

    /// Register a new agent
    pub async fn register_agent(&self, config: AgentConfig) -> AgentId {
        let agent = Agent::new(config);
        let id = agent.id;
        
        let mut agents = self.agents.write().await;
        agents.insert(id, agent);
        
        self.event_bus.publish(Event::AgentRegistered { id }).await;
        id
    }

    /// Get an agent by ID
    pub async fn get_agent(&self, id: &AgentId) -> Option<Agent> {
        let agents = self.agents.read().await;
        agents.get(id).cloned()
    }

    /// List all agents
    pub async fn list_agents(&self) -> Vec<Agent> {
        let agents = self.agents.read().await;
        agents.values().cloned().collect()
    }

    /// Remove an agent
    pub async fn remove_agent(&self, id: &AgentId) -> Option<Agent> {
        let mut agents = self.agents.write().await;
        let agent = agents.remove(id);
        
        if let Some(ref a) = agent {
            self.event_bus.publish(Event::AgentRemoved { id: *id }).await;
        }
        
        agent
    }

    /// Submit a new task
    pub async fn submit_task(&self, task: Task) -> TaskId {
        let id = task.id;
        
        let mut tasks = self.tasks.write().await;
        tasks.insert(id, task);
        
        self.event_bus.publish(Event::TaskSubmitted { id }).await;
        id
    }

    /// Assign a task to an available agent
    pub async fn assign_task_to_agent(&self, task_id: TaskId, agent_id: AgentId) -> Result<(), String> {
        // Check if agent is available
        {
            let mut agents = self.agents.write().await;
            if let Some(agent) = agents.get_mut(&agent_id) {
                agent.assign_task(task_id)
                    .map_err(|e| e.to_string())?;
            } else {
                return Err("Agent not found".to_string());
            }
        }

        // Update task status
        {
            let mut tasks = self.tasks.write().await;
            if let Some(task) = tasks.get_mut(&task_id) {
                task.status = TaskStatus::InProgress;
                task.assigned_to = Some(agent_id);
            }
        }

        self.event_bus.publish(Event::TaskAssigned { 
            task_id, 
            agent_id 
        }).await;

        Ok(())
    }

    /// Auto-assign task to first available agent
    pub async fn auto_assign_task(&self, task_id: TaskId) -> Option<AgentId> {
        let agents = self.agents.read().await;
        
        for (id, agent) in agents.iter() {
            if agent.is_available() {
                drop(agents);
                if self.assign_task_to_agent(task_id, *id).await.is_ok() {
                    return Some(*id);
                }
            }
        }
        
        None
    }

    /// Complete a task
    pub async fn complete_task(&self, task_id: TaskId) -> Result<(), String> {
        let mut tasks = self.tasks.write().await;
        
        if let Some(task) = tasks.get_mut(&task_id) {
            task.status = TaskStatus::Completed;
            
            if let Some(agent_id) = task.assigned_to {
                let mut agents = self.agents.write().await;
                if let Some(agent) = agents.get_mut(&agent_id) {
                    agent.complete_task();
                }
            }
            
            self.event_bus.publish(Event::TaskCompleted { id: task_id }).await;
            Ok(())
        } else {
            Err("Task not found".to_string())
        }
    }

    /// Fail a task
    pub async fn fail_task(&self, task_id: TaskId, error: String) -> Result<(), String> {
        let mut tasks = self.tasks.write().await;
        
        if let Some(task) = tasks.get_mut(&task_id) {
            task.status = TaskStatus::Failed(error.clone());
            
            if let Some(agent_id) = task.assigned_to {
                let mut agents = self.agents.write().await;
                if let Some(agent) = agents.get_mut(&agent_id) {
                    agent.fail_task(error);
                }
            }
            
            self.event_bus.publish(Event::TaskFailed { id: task_id }).await;
            Ok(())
        } else {
            Err("Task not found".to_string())
        }
    }

    /// Get event bus subscriber
    pub fn event_subscriber(&self) -> async_channel::Receiver<Event> {
        self.event_bus.subscribe()
    }

    /// Get statistics
    pub async fn stats(&self) -> OrchestratorStats {
        let agents = self.agents.read().await;
        let tasks = self.tasks.read().await;
        
        let active_agents = agents.values()
            .filter(|a| matches!(a.state, AgentState::Busy))
            .count();
        
        let pending_tasks = tasks.values()
            .filter(|t| matches!(t.status, TaskStatus::Pending))
            .count();
        
        OrchestratorStats {
            total_agents: agents.len(),
            active_agents,
            total_tasks: tasks.len(),
            pending_tasks,
        }
    }
}

/// Orchestrator statistics
#[derive(Debug, Clone)]
pub struct OrchestratorStats {
    pub total_agents: usize,
    pub active_agents: usize,
    pub total_tasks: usize,
    pub pending_tasks: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::TaskPriority;

    #[tokio::test]
    async fn test_orchestrator_basic() {
        let orchestrator = Orchestrator::new(OrchestratorConfig::default());
        
        // Register an agent
        let config = AgentConfig {
            name: "test".to_string(),
            provider: "openai".to_string(),
            model: "gpt-4".to_string(),
            tools: vec![],
            guardrails: vec![],
            schedule: None,
            extra: HashMap::new(),
        };
        
        let agent_id = orchestrator.register_agent(config).await;
        assert!(orchestrator.get_agent(&agent_id).await.is_some());
        
        // Submit a task
        let task = Task::new(
            "test task".to_string(),
            "do something".to_string(),
            TaskPriority::Normal,
        );
        let task_id = orchestrator.submit_task(task).await;
        
        // Auto-assign should work
        let assigned = orchestrator.auto_assign_task(task_id).await;
        assert_eq!(assigned, Some(agent_id));
        
        // Complete the task
        assert!(orchestrator.complete_task(task_id).await.is_ok());
    }
}
