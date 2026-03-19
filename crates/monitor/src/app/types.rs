//! Shared UI state types for the desktop app.

use crate::controller::LiveAttachController;
use nca_common::config::PermissionMode;
use nca_common::event::EndReason;
use nca_common::orchestration::TodoPriority;
use nca_runtime::service::{ServiceSessionHandle, ServiceSessionInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum View {
    Dashboard,
    Projects,
    Todos,
    Agents,
    Chat,
    Git,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsScope {
    Global,
    Project,
}

#[derive(Debug, Clone)]
pub(crate) struct ComposerState {
    pub prompt: String,
    pub model: String,
    pub safe_mode: bool,
    pub permission_mode: PermissionMode,
}

impl Default for ComposerState {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            model: String::new(),
            safe_mode: false,
            permission_mode: PermissionMode::AcceptEdits,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChatRole {
    User,
    Assistant,
    Tool,
    Error,
}

#[derive(Debug, Clone)]
pub(crate) struct ChatEntry {
    pub role: ChatRole,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingApproval {
    pub call_id: String,
    pub tool: String,
    pub description: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CompanyForm {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ProjectForm {
    pub name: String,
    pub slug: String,
    pub description: String,
    pub workspace_root: String,
    /// One task title per line; each becomes a todo after the project is created.
    pub initial_tasks: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TodoForm {
    pub title: String,
    pub description: String,
    pub acceptance_criteria: String,
    pub priority: TodoPriority,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct AgentForm {
    pub name: String,
    pub role: String,
    pub model: String,
    pub prompt_hint: String,
}

pub(crate) struct ActiveSession {
    pub _service_handle: Option<ServiceSessionHandle>,
    pub controller: LiveAttachController,
    pub info: ServiceSessionInfo,
    pub transcript: Vec<ChatEntry>,
    pub pending_approvals: Vec<PendingApproval>,
    pub composer: String,
    pub streaming_assistant: String,
    pub last_error: Option<String>,
    pub run_in_progress: bool,
    pub ended: Option<EndReason>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub estimated_cost_usd: f64,
    pub child_session_ids: Vec<String>,
}

impl ActiveSession {
    pub fn from_loaded(
        info: ServiceSessionInfo,
        controller: LiveAttachController,
        service_handle: Option<ServiceSessionHandle>,
        transcript: Vec<ChatEntry>,
    ) -> Self {
        Self {
            _service_handle: service_handle,
            controller,
            info,
            transcript,
            pending_approvals: Vec::new(),
            composer: String::new(),
            streaming_assistant: String::new(),
            last_error: None,
            run_in_progress: false,
            ended: None,
            input_tokens: 0,
            output_tokens: 0,
            estimated_cost_usd: 0.0,
            child_session_ids: Vec::new(),
        }
    }

    pub fn push_user(&mut self, content: String) {
        self.transcript.push(ChatEntry {
            role: ChatRole::User,
            title: "Developer".into(),
            content,
        });
    }

    pub fn push_assistant(&mut self, content: String) {
        self.streaming_assistant.clear();
        if !content.trim().is_empty() {
            self.transcript.push(ChatEntry {
                role: ChatRole::Assistant,
                title: "Orchestrator".into(),
                content,
            });
        }
    }

    pub fn push_tool(&mut self, content: String) {
        self.transcript.push(ChatEntry {
            role: ChatRole::Tool,
            title: "System".into(),
            content,
        });
    }

    pub fn push_error(&mut self, content: String) {
        self.last_error = Some(content.clone());
        self.transcript.push(ChatEntry {
            role: ChatRole::Error,
            title: "Error".into(),
            content,
        });
    }
}

impl Drop for ActiveSession {
    fn drop(&mut self) {
        self.controller.stop();
    }
}
