//! Planner module - Task planning and decomposition

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PlanError {
    #[error("Failed to parse plan: {0}")]
    ParseError(String),
    #[error("Invalid step: {0}")]
    InvalidStep(String),
}

/// A single step in a plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub id: String,
    pub description: String,
    pub tool: Option<String>,
    pub arguments: Vec<String>,
    pub completed: bool,
    pub result: Option<String>,
}

impl PlanStep {
    pub fn new(description: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            description: description.to_string(),
            tool: None,
            arguments: Vec::new(),
            completed: false,
            result: None,
        }
    }

    pub fn with_tool(mut self, tool: &str, arguments: Vec<String>) -> Self {
        self.tool = Some(tool.to_string());
        self.arguments = arguments;
        self
    }
}

/// A plan consisting of multiple steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    pub goal: String,
    pub steps: Vec<PlanStep>,
    pub current_step_index: usize,
    pub status: PlanStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlanStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

impl Plan {
    pub fn new(goal: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            goal: goal.to_string(),
            steps: Vec::new(),
            current_step_index: 0,
            status: PlanStatus::Pending,
        }
    }

    pub fn add_step(&mut self, step: PlanStep) {
        self.steps.push(step);
    }

    /// Parse a plan from LLM response
    pub fn from_response(response: &str) -> Result<Self, PlanError> {
        let mut plan = Plan::new("Extracted from response");
        
        // Simple parsing: each line is a step
        for line in response.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            // Check for tool usage pattern: "step: tool(arg1, arg2)"
            if let Some(step_desc) = line.strip_prefix("- ") {
                let step = PlanStep::new(step_desc);
                plan.add_step(step);
            } else {
                let step = PlanStep::new(line);
                plan.add_step(step);
            }
        }

        if plan.steps.is_empty() {
            return Err(PlanError::ParseError("No steps found in response".to_string()));
        }

        Ok(plan)
    }

    /// Get the next uncompleted step
    pub fn get_next_step(&self) -> Option<&PlanStep> {
        self.steps.get(self.current_step_index)
    }

    /// Get the next uncompleted step (mutable)
    pub fn get_next_step_mut(&mut self) -> Option<&mut PlanStep> {
        self.steps.get_mut(self.current_step_index)
    }

    /// Mark the current step as completed
    pub fn mark_step_completed(&mut self, step_id: &str) {
        if let Some(step) = self.steps.get_mut(self.current_step_index) {
            if step.id == step_id {
                step.completed = true;
                self.current_step_index += 1;
                
                if self.current_step_index >= self.steps.len() {
                    self.status = PlanStatus::Completed;
                } else {
                    self.status = PlanStatus::InProgress;
                }
            }
        }
    }

    /// Check if all steps are completed
    pub fn is_complete(&self) -> bool {
        self.status == PlanStatus::Completed || 
        self.current_step_index >= self.steps.len()
    }

    /// Get progress percentage
    pub fn progress(&self) -> f32 {
        if self.steps.is_empty() {
            return 0.0;
        }
        (self.current_step_index as f32 / self.steps.len() as f32) * 100.0
    }

    /// Get remaining steps count
    pub fn remaining_steps(&self) -> usize {
        self.steps.len() - self.current_step_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_creation() {
        let mut plan = Plan::new("Test goal");
        plan.add_step(PlanStep::new("Step 1"));
        plan.add_step(PlanStep::new("Step 2"));
        
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.remaining_steps(), 2);
    }

    #[test]
    fn test_plan_progress() {
        let mut plan = Plan::new("Test goal");
        plan.add_step(PlanStep::new("Step 1"));
        plan.add_step(PlanStep::new("Step 2"));
        plan.add_step(PlanStep::new("Step 3"));
        
        assert_eq!(plan.progress(), 0.0);
        
        plan.mark_step_completed(&plan.steps[0].id);
        assert_eq!(plan.progress(), 33.333332);
        
        plan.mark_step_completed(&plan.steps[1].id);
        plan.mark_step_completed(&plan.steps[2].id);
        assert!(plan.is_complete());
    }

    #[test]
    fn test_plan_from_response() {
        let response = r#"
# Plan to accomplish task
- Step 1: Analyze the problem
- Step 2: Implement solution
- Step 3: Test and verify
"#;
        
        let plan = Plan::from_response(response).unwrap();
        assert_eq!(plan.steps.len(), 3);
    }
}
