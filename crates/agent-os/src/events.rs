//! Event-driven communication system

use async_channel::{Sender, Receiver};
use tokio::sync::broadcast;
use serde::{Deserialize, Serialize};
use crate::agent::AgentId;
use crate::task::TaskId;

/// Events in the agent system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    // Agent events
    AgentRegistered { id: AgentId },
    AgentRemoved { id: AgentId },
    AgentStateChanged { id: AgentId, old_state: String, new_state: String },
    
    // Task events
    TaskSubmitted { id: TaskId },
    TaskAssigned { task_id: TaskId, agent_id: AgentId },
    TaskCompleted { id: TaskId },
    TaskFailed { id: TaskId },
    TaskCancelled { id: TaskId },
    
    // System events
    Shutdown,
    Custom { name: String, data: serde_json::Value },
}

/// Event bus for pub/sub communication
pub struct EventBus {
    sender: broadcast::Sender<Event>,
}

impl EventBus {
    /// Create a new event bus
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1000);
        Self { sender }
    }

    /// Publish an event
    pub async fn publish(&self, event: Event) {
        let _ = self.sender.send(event);
    }

    /// Subscribe to all events
    pub fn subscribe(&self) -> Receiver<Event> {
        let receiver = self.sender.subscribe();
        
        // Convert broadcast receiver to async_channel receiver
        let (tx, rx) = async_channel::bounded(1000);
        
        // Spawn task to forward events
        let mut rx_broadcast = receiver.resubscribe();
        tokio::spawn(async move {
            while let Ok(event) = rx_broadcast.recv().await {
                if tx.send(event).await.is_err() {
                    break;
                }
            }
        });
        
        rx
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_bus() {
        let bus = EventBus::new();
        let mut subscriber = bus.subscribe();
        
        // Publish an event
        let event = Event::AgentRegistered { 
            id: AgentId::new_v4() 
        };
        bus.publish(event.clone()).await;
        
        // Receive the event
        let received = subscriber.recv().await.unwrap();
        match received {
            Event::AgentRegistered { id } => {
                assert_eq!(id, match event { Event::AgentRegistered { id } => id });
            }
            _ => panic!("Wrong event type"),
        }
    }
}
