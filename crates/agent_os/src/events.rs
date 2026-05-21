//! Events module - Event system for agent communication

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Event types in the agent system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    // Agent lifecycle events
    AgentCreated,
    AgentStarted,
    AgentPaused,
    AgentResumed,
    AgentCompleted,
    AgentFailed,
    
    // Planning events
    PlanCreated,
    PlanUpdated,
    StepStarted,
    StepCompleted,
    StepFailed,
    
    // Tool events
    ToolCalled,
    ToolCompleted,
    ToolFailed,
    
    // Memory events
    MemoryAdded,
    MemoryRetrieved,
    MemoryUpdated,
    
    // Communication events
    MessageReceived,
    MessageSent,
    Broadcast,
    
    // System events
    Error,
    Warning,
    Info,
    Log,
}

/// An event in the agent system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub event_type: EventType,
    pub source: String,
    pub target: Option<String>,
    pub payload: serde_json::Value,
    pub timestamp: DateTime<Utc>,
    pub correlation_id: Option<String>,
}

impl Event {
    pub fn new(event_type: EventType, source: &str, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            event_type,
            source: source.to_string(),
            target: None,
            payload,
            timestamp: Utc::now(),
            correlation_id: None,
        }
    }

    pub fn with_target(mut self, target: &str) -> Self {
        self.target = Some(target.to_string());
        self
    }

    pub fn with_correlation(mut self, correlation_id: &str) -> Self {
        self.correlation_id = Some(correlation_id.to_string());
        self
    }

    /// Create a builder-style event
    pub fn builder(event_type: EventType, source: &str) -> EventBuilder {
        EventBuilder::new(event_type, source)
    }
}

/// Event builder for fluent API
pub struct EventBuilder {
    event_type: EventType,
    source: String,
    target: Option<String>,
    payload: serde_json::Value,
    correlation_id: Option<String>,
}

impl EventBuilder {
    pub fn new(event_type: EventType, source: &str) -> Self {
        Self {
            event_type,
            source: source.to_string(),
            target: None,
            payload: serde_json::Value::Null,
            correlation_id: None,
        }
    }

    pub fn target(mut self, target: &str) -> Self {
        self.target = Some(target.to_string());
        self
    }

    pub fn payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = payload;
        self
    }

    pub fn correlation(mut self, correlation_id: &str) -> Self {
        self.correlation_id = Some(correlation_id.to_string());
        self
    }

    pub fn build(self) -> Event {
        Event {
            id: Uuid::new_v4().to_string(),
            event_type: self.event_type,
            source: self.source,
            target: self.target,
            payload: self.payload,
            timestamp: Utc::now(),
            correlation_id: self.correlation_id,
        }
    }
}

/// Event handler trait
pub trait EventHandler: Send + Sync {
    fn handle(&self, event: &Event);
}

/// Simple event bus for intra-process event communication
pub struct EventBus {
    handlers: Vec<Box<dyn EventHandler>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    pub fn register(&mut self, handler: Box<dyn EventHandler>) {
        self.handlers.push(handler);
    }

    pub fn publish(&self, event: &Event) {
        for handler in &self.handlers {
            handler.handle(event);
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

// Convenience functions for creating common events
impl Event {
    pub fn agent_started(agent_id: &str, task: &str) -> Self {
        Self::builder(EventType::AgentStarted, agent_id)
            .payload(serde_json::json!({"task": task}))
            .build()
    }

    pub fn plan_created(agent_id: &str, plan_id: &str, steps: usize) -> Self {
        Self::builder(EventType::PlanCreated, agent_id)
            .payload(serde_json::json!({
                "plan_id": plan_id,
                "steps": steps
            }))
            .build()
    }

    pub fn step_completed(agent_id: &str, step_id: &str, result: &str) -> Self {
        Self::builder(EventType::StepCompleted, agent_id)
            .payload(serde_json::json!({
                "step_id": step_id,
                "result": result
            }))
            .build()
    }

    pub fn tool_called(agent_id: &str, tool_name: &str, args: &serde_json::Value) -> Self {
        Self::builder(EventType::ToolCalled, agent_id)
            .payload(serde_json::json!({
                "tool": tool_name,
                "args": args
            }))
            .build()
    }

    pub fn error(source: &str, message: &str) -> Self {
        Self::builder(EventType::Error, source)
            .payload(serde_json::json!({"message": message}))
            .build()
    }

    pub fn info(source: &str, message: &str) -> Self {
        Self::builder(EventType::Info, source)
            .payload(serde_json::json!({"message": message}))
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let event = Event::new(EventType::Info, "test", serde_json::json!({"msg": "hello"}));
        
        assert_eq!(event.source, "test");
        assert!(matches!(event.event_type, EventType::Info));
    }

    #[test]
    fn test_event_builder() {
        let event = Event::builder(EventType::AgentStarted, "agent-1")
            .target("user-1")
            .payload(serde_json::json!({"task": "test"}))
            .correlation("corr-123")
            .build();

        assert_eq!(event.target, Some("user-1".to_string()));
        assert_eq!(event.correlation_id, Some("corr-123".to_string()));
    }

    #[test]
    fn test_convenience_events() {
        let event = Event::agent_started("agent-1", "test task");
        assert!(matches!(event.event_type, EventType::AgentStarted));
    }
}
