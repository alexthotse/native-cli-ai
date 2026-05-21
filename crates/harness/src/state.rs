//! Global state management for the harness engine

use std::sync::Arc;
use dashmap::DashMap;
use uuid::Uuid;
use parking_lot::RwLock;

/// Represents the current state of a task
#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Reviewing,
    Testing,
    Completed,
    Failed(String),
}

/// Represents a single task in the harness
#[derive(Debug, Clone)]
pub struct Task {
    pub id: Uuid,
    pub description: String,
    pub status: TaskStatus,
    pub attempts: u32,
    pub created_at: std::time::SystemTime,
    pub updated_at: std::time::SystemTime,
}

impl Task {
    pub fn new(description: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            description,
            status: TaskStatus::Pending,
            attempts: 0,
            created_at: std::time::SystemTime::now(),
            updated_at: std::time::SystemTime::now(),
        }
    }
}

/// Global state for the harness engine
pub struct HarnessState {
    /// Map of task ID to task
    pub tasks: DashMap<Uuid, Task>,
    
    /// Current working directory
    pub workdir: String,
    
    /// Compilation errors encountered
    pub errors: RwLock<Vec<String>>,
    
    /// Successfully applied patches
    pub patches: RwLock<Vec<String>>,
    
    /// Agent conversation history
    pub history: DashMap<String, Vec<String>>,
}

impl Default for HarnessState {
    fn default() -> Self {
        Self::new()
    }
}

impl HarnessState {
    pub fn new() -> Self {
        Self {
            tasks: DashMap::new(),
            workdir: std::env::current_dir()
                .unwrap_or_else(|_| ".".to_string())
                .to_string_lossy()
                .to_string(),
            errors: RwLock::new(Vec::new()),
            patches: RwLock::new(Vec::new()),
            history: DashMap::new(),
        }
    }
    
    pub fn add_task(&self, description: String) -> Uuid {
        let task = Task::new(description);
        let id = task.id;
        self.tasks.insert(id, task);
        id
    }
    
    pub fn get_task(&self, id: &Uuid) -> Option<Task> {
        self.tasks.get(id).map(|r| r.clone())
    }
    
    pub fn update_task_status(&self, id: &Uuid, status: TaskStatus) -> bool {
        if let Some(mut task) = self.tasks.get_mut(id) {
            task.status = status;
            task.updated_at = std::time::SystemTime::now();
            true
        } else {
            false
        }
    }
    
    pub fn increment_attempts(&self, id: &Uuid) {
        if let Some(mut task) = self.tasks.get_mut(id) {
            task.attempts += 1;
        }
    }
    
    pub fn add_error(&self, error: String) {
        self.errors.write().push(error);
    }
    
    pub fn clear_errors(&self) {
        self.errors.write().clear();
    }
    
    pub fn get_errors(&self) -> Vec<String> {
        self.errors.read().clone()
    }
    
    pub fn add_patch(&self, patch: String) {
        self.patches.write().push(patch);
    }
    
    pub fn add_history(&self, agent: String, message: String) {
        self.history
            .entry(agent)
            .or_insert_with(Vec::new)
            .push(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_state_creation() {
        let state = HarnessState::new();
        assert!(!state.workdir.is_empty());
    }
    
    #[test]
    fn test_task_management() {
        let state = HarnessState::new();
        let task_id = state.add_task("Test task".to_string());
        
        let task = state.get_task(&task_id).unwrap();
        assert_eq!(task.description, "Test task");
        assert_eq!(task.status, TaskStatus::Pending);
        
        state.update_task_status(&task_id, TaskStatus::InProgress);
        let updated = state.get_task(&task_id).unwrap();
        assert_eq!(updated.status, TaskStatus::InProgress);
    }
    
    #[test]
    fn test_error_tracking() {
        let state = HarnessState::new();
        state.add_error("Error 1".to_string());
        state.add_error("Error 2".to_string());
        
        let errors = state.get_errors();
        assert_eq!(errors.len(), 2);
        
        state.clear_errors();
        assert!(state.get_errors().is_empty());
    }
}
