//! NCA Agent OS - Multi-agent orchestration system
//! 
//! Inspired by OpenFang's "Hands" concept, this crate provides:
//! - Agent lifecycle management
//! - Task decomposition and assignment
//! - Inter-agent communication
//! - Shared memory and context management

pub mod agent;
pub mod orchestrator;
pub mod task;
pub mod memory;
pub mod events;

pub use agent::Agent;
pub use orchestrator::Orchestrator;
pub use task::Task;
pub use memory::Memory;
