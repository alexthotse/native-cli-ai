//! Ultimate Harness Engineering Engine
//! 
//! This crate provides self-healing, multi-agent development capabilities
//! inspired by free-code and enhanced with ultimate harness engineering.

pub mod engine;
pub mod swarm;
pub mod sandbox;
pub mod synthesizer;
pub mod state;
pub mod tools;

pub use engine::HarnessEngine;
pub use swarm::AgentSwarm;
pub use sandbox::Sandbox;
pub use synthesizer::ToolSynthesizer;
pub use state::HarnessState;
