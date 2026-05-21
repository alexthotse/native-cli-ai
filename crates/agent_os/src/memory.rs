//! Memory module - Agent memory and context management

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Memory entry types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryType {
    Observation(String),
    Action(String),
    Reflection(String),
    Plan(String),
    Result(String),
}

/// A single memory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: usize,
    pub entry_type: MemoryType,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub importance: f32,
}

/// Agent memory system
pub struct Memory {
    entries: Vec<MemoryEntry>,
    next_id: usize,
    max_capacity: usize,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 0,
            max_capacity: 1000,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            next_id: 0,
            max_capacity: capacity,
        }
    }

    /// Add an observation to memory
    pub fn add_observation(&mut self, content: &str) {
        self.add_entry(MemoryType::Observation(content.to_string()), 0.5);
    }

    /// Add an action to memory
    pub fn add_action(&mut self, content: &str) {
        self.add_entry(MemoryType::Action(content.to_string()), 0.7);
    }

    /// Add a reflection to memory
    pub fn add_reflection(&mut self, content: &str) {
        self.add_entry(MemoryType::Reflection(content.to_string()), 0.9);
    }

    /// Add a plan to memory
    pub fn add_plan(&mut self, content: &str) {
        self.add_entry(MemoryType::Plan(content.to_string()), 0.8);
    }

    /// Add a result to memory
    pub fn add_result(&mut self, content: &str) {
        self.add_entry(MemoryType::Result(content.to_string()), 0.6);
    }

    fn add_entry(&mut self, entry_type: MemoryType, importance: f32) {
        let entry = MemoryEntry {
            id: self.next_id,
            entry_type,
            content: String::new(),
            timestamp: Utc::now(),
            importance,
        };

        // Enforce capacity limit by removing oldest/least important entries
        if self.entries.len() >= self.max_capacity {
            self.entries.sort_by(|a, b| {
                b.importance.partial_cmp(&a.importance).unwrap()
            });
            self.entries.truncate(self.max_capacity - 1);
        }

        self.entries.push(entry);
        self.next_id += 1;
    }

    /// Get all memory entries as a formatted string
    pub fn get_history(&self) -> String {
        self.entries
            .iter()
            .map(|e| format!("[{:?}] {}", e.entry_type, e.content))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Get a summary of recent memory
    pub fn get_summary(&self) -> String {
        let recent: Vec<_> = self.entries.iter().rev().take(10).collect();
        recent
            .iter()
            .map(|e| format!("- {}", e.content))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Search memory by keyword
    pub fn search(&self, query: &str) -> Vec<&MemoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.content.to_lowercase().contains(&query.to_lowercase()))
            .collect()
    }

    /// Get entries by type
    pub fn get_by_type(&self, memory_type: &MemoryType) -> Vec<&MemoryEntry> {
        self.entries
            .iter()
            .filter(|e| std::mem::discriminant(&e.entry_type) == std::mem::discriminant(memory_type))
            .collect()
    }

    /// Clear all memories
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get entry count
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_operations() {
        let mut memory = Memory::new();
        
        memory.add_observation("Test observation");
        memory.add_action("Test action");
        memory.add_reflection("Test reflection");
        
        assert_eq!(memory.len(), 3);
        
        let history = memory.get_history();
        assert!(history.contains("Test observation"));
        assert!(history.contains("Test action"));
        assert!(history.contains("Test reflection"));
    }

    #[test]
    fn test_memory_search() {
        let mut memory = Memory::new();
        
        memory.add_observation("Python code example");
        memory.add_observation("Rust implementation");
        
        let results = memory.search("python");
        assert_eq!(results.len(), 1);
    }
}
