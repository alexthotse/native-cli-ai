//! Agent OS - Autonomous AI Agent Framework
//! 
//! Inspired by OpenFang (https://github.com/RightNow-AI/openfang)
//! 
//! This crate provides a comprehensive agent operating system that enables:
//! - Autonomous task planning and execution
//! - Multi-agent collaboration
//! - Tool integration and orchestration
//! - Memory management and context awareness
//! - Self-improvement and learning capabilities

pub mod agent;
pub mod planner;
pub mod memory;
pub mod tools;
pub mod orchestrator;
pub mod events;

pub use agent::Agent;
pub use planner::Planner;
pub use memory::Memory;
pub use tools::{Tool, ToolRegistry};
pub use orchestrator::Orchestrator;
pub use events::{Event, EventType};

/// Agent OS configuration
#[derive(Debug, Clone)]
pub struct AgentOsConfig {
    pub max_iterations: usize,
    pub reflection_enabled: bool,
    pub learning_enabled: bool,
    pub multi_agent_enabled: bool,
}

impl Default for AgentOsConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            reflection_enabled: true,
            learning_enabled: true,
            multi_agent_enabled: false,
        }
    }
}
