//! Task definition and management

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Unique identifier for a task
pub type TaskId = Uuid;

/// Agent ID reference
use crate::agent::AgentId;

/// Task priority levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl Default for TaskPriority {
    fn default() -> Self {
        TaskPriority::Normal
    }
}

/// Task status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    /// Task is waiting to be assigned
    Pending,
    /// Task is being executed by an agent
    InProgress,
    /// Task completed successfully
    Completed,
    /// Task failed with an error message
    Failed(String),
    /// Task is waiting for approval
    AwaitingApproval,
    /// Task was cancelled
    Cancelled,
}

/// Task definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique task identifier
    pub id: TaskId,
    /// Human-readable title
    pub title: String,
    /// Detailed description of what needs to be done
    pub description: String,
    /// Task priority
    pub priority: TaskPriority,
    /// Current status
    pub status: TaskStatus,
    /// Agent assigned to this task (if any)
    pub assigned_to: Option<AgentId>,
    /// Parent task ID (for subtasks)
    pub parent_id: Option<TaskId>,
    /// Child task IDs
    pub subtasks: Vec<TaskId>,
    /// Required tools for this task
    pub required_tools: Vec<String>,
    /// Task metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// When the task was created
    pub created_at: DateTime<Utc>,
    /// When the task was last updated
    pub updated_at: DateTime<Utc>,
    /// When the task was completed (if applicable)
    pub completed_at: Option<DateTime<Utc>>,
}

impl Task {
    /// Create a new task
    pub fn new(title: String, description: String, priority: TaskPriority) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            title,
            description,
            priority,
            status: TaskStatus::Pending,
            assigned_to: None,
            parent_id: None,
            subtasks: vec![],
            required_tools: vec![],
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
            completed_at: None,
        }
    }

    /// Add a subtask
    pub fn add_subtask(&mut self, subtask_id: TaskId) {
        self.subtasks.push(subtask_id);
    }

    /// Set parent task
    pub fn set_parent(&mut self, parent_id: TaskId) {
        self.parent_id = Some(parent_id);
    }

    /// Mark task as requiring specific tools
    pub fn require_tools(&mut self, tools: Vec<String>) {
        self.required_tools = tools;
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: &str, value: serde_json::Value) -> Self {
        self.metadata.insert(key.to_string(), value);
        self
    }

    /// Check if task is complete
    pub fn is_complete(&self) -> bool {
        matches!(self.status, TaskStatus::Completed)
    }

    /// Check if task has failed
    pub fn is_failed(&self) -> bool {
        matches!(self.status, TaskStatus::Failed(_))
    }

    /// Check if task is pending
    pub fn is_pending(&self) -> bool {
        matches!(self.status, TaskStatus::Pending)
    }
}

/// Task decomposition result
#[derive(Debug, Clone)]
pub struct TaskDecomposition {
    /// Original task
    pub original: Task,
    /// Decomposed subtasks
    pub subtasks: Vec<Task>,
}

impl TaskDecomposition {
    /// Create a decomposition with linked parent-child relationships
    pub fn new(original: Task, mut subtasks: Vec<Task>) -> Self {
        let original_id = original.id;
        
        // Link subtasks to parent
        for subtask in &mut subtasks {
            subtask.set_parent(original_id);
        }
        
        // Link parent to subtasks
        let mut original_with_subtasks = original;
        for subtask in &subtasks {
            original_with_subtasks.add_subtask(subtask.id);
        }
        
        Self {
            original: original_with_subtasks,
            subtasks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_creation() {
        let task = Task::new(
            "Test Task".to_string(),
            "Do something important".to_string(),
            TaskPriority::High,
        );
        
        assert_eq!(task.title, "Test Task");
        assert_eq!(task.priority, TaskPriority::High);
        assert_eq!(task.status, TaskStatus::Pending);
        assert!(task.assigned_to.is_none());
    }

    #[test]
    fn test_task_subtasks() {
        let mut parent = Task::new(
            "Parent".to_string(),
            "Parent task".to_string(),
            TaskPriority::Normal,
        );
        
        let child1 = Task::new(
            "Child 1".to_string(),
            "First subtask".to_string(),
            TaskPriority::Normal,
        );
        
        let child2 = Task::new(
            "Child 2".to_string(),
            "Second subtask".to_string(),
            TaskPriority::Normal,
        );
        
        parent.add_subtask(child1.id);
        parent.add_subtask(child2.id);
        
        assert_eq!(parent.subtasks.len(), 2);
        assert!(parent.subtasks.contains(&child1.id));
        assert!(parent.subtasks.contains(&child2.id));
    }

    #[test]
    fn test_task_decomposition() {
        let original = Task::new(
            "Build feature".to_string(),
            "Implement the new feature".to_string(),
            TaskPriority::High,
        );
        
        let subtasks = vec![
            Task::new("Design".to_string(), "Design the feature".to_string(), TaskPriority::Normal),
            Task::new("Implement".to_string(), "Write the code".to_string(), TaskPriority::Normal),
            Task::new("Test".to_string(), "Write tests".to_string(), TaskPriority::Normal),
        ];
        
        let decomposition = TaskDecomposition::new(original, subtasks);
        
        // Verify parent-child links
        for subtask in &decomposition.subtasks {
            assert_eq!(subtask.parent_id, Some(decomposition.original.id));
        }
        
        assert_eq!(decomposition.original.subtasks.len(), 3);
    }
}
