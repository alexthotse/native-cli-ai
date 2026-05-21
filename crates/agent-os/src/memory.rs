//! Memory management for agents

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// Memory entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Unique key for the memory
    pub key: String,
    /// The actual memory content
    pub value: String,
    /// Memory type (short-term, long-term, episodic, semantic)
    pub memory_type: MemoryType,
    /// Importance score (0.0 - 1.0)
    pub importance: f32,
    /// When the memory was created
    pub created_at: DateTime<Utc>,
    /// Last accessed time
    pub last_accessed: DateTime<Utc>,
    /// Access count
    pub access_count: u64,
}

/// Types of memory
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemoryType {
    /// Short-term working memory (ephemeral)
    ShortTerm,
    /// Long-term persistent memory
    LongTerm,
    /// Episodic memories (specific events)
    Episodic,
    /// Semantic memories (facts and knowledge)
    Semantic,
    /// Procedural memories (how to do things)
    Procedural,
}

/// Memory store for an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    /// All memory entries
    entries: HashMap<String, MemoryEntry>,
    /// Maximum short-term memories before forgetting
    max_short_term: usize,
    /// Minimum importance for long-term storage
    long_term_threshold: f32,
}

impl Memory {
    /// Create a new memory store
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            max_short_term: 100,
            long_term_threshold: 0.7,
        }
    }

    /// Create with custom limits
    pub fn with_limits(max_short_term: usize, long_term_threshold: f32) -> Self {
        Self {
            entries: HashMap::new(),
            max_short_term,
            long_term_threshold,
        }
    }

    /// Store a memory
    pub fn store(&mut self, key: &str, value: &str, memory_type: MemoryType, importance: f32) {
        let now = Utc::now();
        let entry = MemoryEntry {
            key: key.to_string(),
            value: value.to_string(),
            memory_type,
            importance: importance.clamp(0.0, 1.0),
            created_at: now,
            last_accessed: now,
            access_count: 0,
        };

        // Check if we need to forget old short-term memories
        if memory_type == MemoryType::ShortTerm {
            self.forget_old_short_term();
        }

        self.entries.insert(key.to_string(), entry);
    }

    /// Retrieve a memory by key
    pub fn retrieve(&mut self, key: &str) -> Option<&MemoryEntry> {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.access_count += 1;
            entry.last_accessed = Utc::now();
            Some(entry)
        } else {
            None
        }
    }

    /// Search memories by type
    pub fn search_by_type(&self, memory_type: &MemoryType) -> Vec<&MemoryEntry> {
        self.entries
            .values()
            .filter(|e| &e.memory_type == memory_type)
            .collect()
    }

    /// Get most important memories
    pub fn get_important_memories(&self, limit: usize) -> Vec<&MemoryEntry> {
        let mut entries: Vec<&MemoryEntry> = self.entries.values().collect();
        entries.sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap());
        entries.into_iter().take(limit).collect()
    }

    /// Delete a memory
    pub fn delete(&mut self, key: &str) -> bool {
        self.entries.remove(key).is_some()
    }

    /// Clear all short-term memories
    pub fn clear_short_term(&mut self) {
        self.entries.retain(|_, e| e.memory_type != MemoryType::ShortTerm);
    }

    /// Forget oldest short-term memories if over limit
    fn forget_old_short_term(&mut self) {
        let short_term_count = self.entries
            .values()
            .filter(|e| e.memory_type == MemoryType::ShortTerm)
            .count();

        if short_term_count > self.max_short_term {
            // Remove oldest short-term memories
            let mut short_terms: Vec<(String, DateTime<Utc>)> = self.entries
                .iter()
                .filter(|(_, e)| e.memory_type == MemoryType::ShortTerm)
                .map(|(k, e)| (k.clone(), e.created_at))
                .collect();
            
            short_terms.sort_by_key(|(_, t)| *t);
            
            let to_remove = short_term_count - self.max_short_term;
            for (key, _) in short_terms.into_iter().take(to_remove) {
                self.entries.remove(&key);
            }
        }
    }

    /// Get all memories as context string
    pub fn as_context(&self, limit: Option<usize>) -> String {
        let mut entries: Vec<&MemoryEntry> = self.entries.values().collect();
        entries.sort_by(|a, b| {
            b.importance.partial_cmp(&a.importance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if let Some(n) = limit {
            entries.truncate(n);
        }

        entries
            .iter()
            .map(|e| format!("[{}] {}", e.memory_type_as_str(), e.value))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Get memory statistics
    pub fn stats(&self) -> MemoryStats {
        let mut by_type = HashMap::new();
        for entry in self.entries.values() {
            *by_type.entry(entry.memory_type.clone()).or_insert(0) += 1;
        }

        MemoryStats {
            total_count: self.entries.len(),
            by_type,
            avg_importance: if self.entries.is_empty() {
                0.0
            } else {
                self.entries.values().map(|e| e.importance).sum::<f32>() / self.entries.len() as f32
            },
        }
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryEntry {
    /// Get memory type as string
    pub fn memory_type_as_str(&self) -> &'static str {
        match self.memory_type {
            MemoryType::ShortTerm => "SHORT",
            MemoryType::LongTerm => "LONG",
            MemoryType::Episodic => "EPISODIC",
            MemoryType::Semantic => "SEMANTIC",
            MemoryType::Procedural => "PROCEDURAL",
        }
    }
}

/// Memory statistics
#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub total_count: usize,
    pub by_type: HashMap<MemoryType, usize>,
    pub avg_importance: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_store_retrieve() {
        let mut memory = Memory::new();
        
        memory.store("fact1", "The sky is blue", MemoryType::Semantic, 0.8);
        memory.store("event1", "Had coffee this morning", MemoryType::Episodic, 0.5);
        
        let fact = memory.retrieve("fact1").unwrap();
        assert_eq!(fact.value, "The sky is blue");
        assert_eq!(fact.memory_type, MemoryType::Semantic);
        assert_eq!(fact.access_count, 1);
        
        // Retrieve again to check access count
        memory.retrieve("fact1");
        let fact = memory.retrieve("fact1").unwrap();
        assert_eq!(fact.access_count, 3);
    }

    #[test]
    fn test_memory_forgetting() {
        let mut memory = Memory::with_limits(3, 0.7);
        
        // Add 5 short-term memories
        for i in 0..5 {
            memory.store(&format!("mem{}", i), &format!("Value {}", i), MemoryType::ShortTerm, 0.5);
        }
        
        // Should only keep 3
        let short_terms = memory.search_by_type(&MemoryType::ShortTerm);
        assert_eq!(short_terms.len(), 3);
    }

    #[test]
    fn test_memory_importance_sorting() {
        let mut memory = Memory::new();
        
        memory.store("low", "Low importance", MemoryType::LongTerm, 0.3);
        memory.store("high", "High importance", MemoryType::LongTerm, 0.9);
        memory.store("med", "Medium importance", MemoryType::LongTerm, 0.6);
        
        let important = memory.get_important_memories(2);
        assert_eq!(important.len(), 2);
        assert_eq!(important[0].value, "High importance");
        assert_eq!(important[1].value, "Medium importance");
    }
}
