//! Multi-agent swarm coordination

use std::sync::Arc;
use agent_os::agent::{Agent, AgentRole};
use agent_os::orchestrator::Orchestrator;
use crate::state::{HarnessState, TaskStatus};
use uuid::Uuid;

/// Specialized agent roles for development
#[derive(Debug, Clone)]
pub enum DeveloperRole {
    Architect,
    Coder,
    Reviewer,
    Tester,
    Debugger,
}

impl DeveloperRole {
    pub fn to_agent_role(&self) -> AgentRole {
        match self {
            DeveloperRole::Architect => AgentRole::Assistant(
                "You are a software architect. Design system structure, break down tasks, and plan implementation.".to_string()
            ),
            DeveloperRole::Coder => AgentRole::Assistant(
                "You are an expert coder. Write clean, efficient, well-documented code following best practices.".to_string()
            ),
            DeveloperRole::Reviewer => AgentRole::Assistant(
                "You are a code reviewer. Review code for quality, security, performance, and adherence to standards.".to_string()
            ),
            DeveloperRole::Tester => AgentRole::Assistant(
                "You are a testing expert. Write comprehensive tests and verify functionality.".to_string()
            ),
            DeveloperRole::Debugger => AgentRole::Assistant(
                "You are a debugging specialist. Analyze errors and provide fixes.".to_string()
            ),
        }
    }
}

/// Agent in the swarm
pub struct SwarmAgent {
    pub id: Uuid,
    pub role: DeveloperRole,
    pub agent: Agent,
    pub active: bool,
}

impl SwarmAgent {
    pub fn new(role: DeveloperRole, name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: role.clone(),
            agent: Agent::new(name, role.to_agent_role()),
            active: true,
        }
    }
}

/// Multi-agent swarm for collaborative development
pub struct AgentSwarm {
    pub agents: Vec<SwarmAgent>,
    pub orchestrator: Orchestrator,
    pub state: Arc<HarnessState>,
}

impl AgentSwarm {
    pub fn new(state: Arc<HarnessState>) -> Self {
        let mut swarm = Self {
            agents: Vec::new(),
            orchestrator: Orchestrator::new(),
            state,
        };
        
        // Initialize default swarm
        swarm.spawn_agent(DeveloperRole::Architect, "architect");
        swarm.spawn_agent(DeveloperRole::Coder, "coder");
        swarm.spawn_agent(DeveloperRole::Reviewer, "reviewer");
        swarm.spawn_agent(DeveloperRole::Tester, "tester");
        swarm.spawn_agent(DeveloperRole::Debugger, "debugger");
        
        swarm
    }
    
    pub fn spawn_agent(&mut self, role: DeveloperRole, name: &str) -> Uuid {
        let agent = SwarmAgent::new(role, name.to_string());
        let id = agent.id;
        self.agents.push(agent);
        id
    }
    
    pub fn get_active_agents(&self) -> Vec<&SwarmAgent> {
        self.agents.iter().filter(|a| a.active).collect()
    }
    
    pub fn deactivate_agent(&mut self, id: &Uuid) {
        if let Some(agent) = self.agents.iter_mut().find(|a| &a.id == id) {
            agent.active = false;
        }
    }
    
    /// Execute a task with the swarm
    pub async fn execute_task(&self, task_id: &Uuid, description: &str) -> Result<String, String> {
        self.state.update_task_status(task_id, TaskStatus::InProgress);
        self.state.add_history("swarm".to_string(), format!("Starting task: {}", description));
        
        // Phase 1: Architect plans
        let architect = self.get_agent_by_role(&DeveloperRole::Architect)?;
        let plan = architect.agent.process(description).await
            .map_err(|e| e.to_string())?;
        self.state.add_history("architect".to_string(), plan.clone());
        
        // Phase 2: Coder implements
        let coder = self.get_agent_by_role(&DeveloperRole::Coder)?;
        let code = coder.agent.process(&format!("Implement this plan: {}", plan)).await
            .map_err(|e| e.to_string())?;
        self.state.add_history("coder".to_string(), code.clone());
        
        // Phase 3: Reviewer checks
        let reviewer = self.get_agent_by_role(&DeveloperRole::Reviewer)?;
        let review = reviewer.agent.process(&format!("Review this code: {}", code)).await
            .map_err(|e| e.to_string())?;
        self.state.add_history("reviewer".to_string(), review.clone());
        
        // Phase 4: Tester validates
        let tester = self.get_agent_by_role(&DeveloperRole::Tester)?;
        let tests = tester.agent.process(&format!("Write tests for: {}", code)).await
            .map_err(|e| e.to_string())?;
        self.state.add_history("tester".to_string(), tests.clone());
        
        self.state.update_task_status(task_id, TaskStatus::Completed);
        
        Ok(format!("Plan: {}\nCode: {}\nReview: {}\nTests: {}", plan, code, review, tests))
    }
    
    fn get_agent_by_role(&self, role: &DeveloperRole) -> Result<&SwarmAgent, String> {
        self.agents
            .iter()
            .find(|a| matches!((&a.role, role), (DeveloperRole::Architect, DeveloperRole::Architect) | 
                                              (DeveloperRole::Coder, DeveloperRole::Coder) |
                                              (DeveloperRole::Reviewer, DeveloperRole::Reviewer) |
                                              (DeveloperRole::Tester, DeveloperRole::Tester) |
                                              (DeveloperRole::Debugger, DeveloperRole::Debugger)))
            .ok_or_else(|| "Agent not found".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_swarm_creation() {
        let state = Arc::new(HarnessState::new());
        let swarm = AgentSwarm::new(state);
        assert_eq!(swarm.agents.len(), 5);
    }
    
    #[test]
    fn test_spawn_agent() {
        let state = Arc::new(HarnessState::new());
        let mut swarm = AgentSwarm::new(state);
        let id = swarm.spawn_agent(DeveloperRole::Debugger, "extra_debugger");
        assert_eq!(swarm.agents.len(), 6);
        assert!(swarm.get_active_agents().iter().any(|a| a.id == id));
    }
}
