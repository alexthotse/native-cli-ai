//! Transcript + status driven by `AgentEvent`.

use nca_common::event::AgentEvent;
use std::time::Instant;

#[derive(Debug, Clone)]
pub enum DisplayBlock {
    User(String),
    Assistant(String),
    ToolRunning { name: String, call_id: String },
    ToolDone {
        name: String,
        ok: bool,
        detail: String,
    },
    System(String),
    ErrorLine(String),
}

pub struct TuiSessionState {
    pub blocks: Vec<DisplayBlock>,
    /// In-progress assistant text (shown below committed blocks until finalized).
    pub streaming_assistant: Option<String>,
    pub input_buffer: String,
    pub cursor_char_idx: usize,
    /// Scroll offset in *lines* (flattened transcript).
    pub scroll_lines: usize,
    pub session_id: String,
    pub model: String,
    pub agent_profile: String,
    pub permission_mode: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    pub started: Instant,
    pub busy: bool,
    pub should_exit: bool,
    /// Selected row in slash-command popup (↑↓ or click).
    pub slash_menu_index: usize,
}

impl TuiSessionState {
    pub fn new(
        session_id: String,
        model: String,
        agent_profile: String,
        permission_mode: String,
    ) -> Self {
        Self {
            blocks: Vec::new(),
            streaming_assistant: None,
            input_buffer: String::new(),
            cursor_char_idx: 0,
            scroll_lines: 0,
            session_id,
            model,
            agent_profile,
            permission_mode,
            input_tokens: 0,
            output_tokens: 0,
            cost_usd: 0.0,
            started: Instant::now(),
            busy: false,
            should_exit: false,
            slash_menu_index: 0,
        }
    }

    pub fn set_busy(&mut self, busy: bool) {
        self.busy = busy;
    }

    pub fn push_error(&mut self, msg: String) {
        self.blocks.push(DisplayBlock::ErrorLine(msg));
    }

    pub fn set_agent_profile(&mut self, label: &str) {
        self.agent_profile = label.to_string();
    }

    pub fn set_permission_mode(&mut self, mode: &str) {
        self.permission_mode = mode.to_string();
    }

    fn flush_stream_before_tool(&mut self) {
        if let Some(s) = self.streaming_assistant.take() {
            if !s.trim().is_empty() {
                self.blocks.push(DisplayBlock::Assistant(s));
            }
        }
    }

    pub fn apply_event(&mut self, e: &AgentEvent) {
        match e {
            AgentEvent::SessionStarted {
                session_id,
                model,
                ..
            } => {
                self.session_id = session_id.clone();
                self.model = model.clone();
            }
            AgentEvent::MessageReceived { role, content } => {
                if role == "user" {
                    self.streaming_assistant = None;
                    self.blocks.push(DisplayBlock::User(content.clone()));
                } else if role == "assistant" {
                    self.streaming_assistant = None;
                    self.blocks.push(DisplayBlock::Assistant(content.clone()));
                }
            }
            AgentEvent::TokensStreamed { delta } => {
                self.streaming_assistant
                    .get_or_insert_with(String::new)
                    .push_str(delta);
            }
            AgentEvent::ToolCallStarted {
                call_id,
                tool,
                ..
            } => {
                self.flush_stream_before_tool();
                self.blocks.push(DisplayBlock::ToolRunning {
                    name: tool.clone(),
                    call_id: call_id.clone(),
                });
            }
            AgentEvent::ToolCallCompleted { call_id, output } => {
                let ok = output.success;
                let detail = if ok {
                    truncate(&output.output, 120)
                } else {
                    output
                        .error
                        .clone()
                        .unwrap_or_else(|| "failed".into())
                };
                if let Some(idx) = self.blocks.iter().rposition(|b| {
                    matches!(b, DisplayBlock::ToolRunning { call_id: id, .. } if id == call_id)
                }) {
                    let name = match &self.blocks[idx] {
                        DisplayBlock::ToolRunning { name, .. } => name.clone(),
                        _ => "?".into(),
                    };
                    self.blocks[idx] = DisplayBlock::ToolDone { name, ok, detail };
                } else {
                    self.blocks.push(DisplayBlock::ToolDone {
                        name: "?".into(),
                        ok,
                        detail,
                    });
                }
            }
            AgentEvent::ApprovalRequested {
                tool, description, ..
            } => {
                self.blocks.push(DisplayBlock::System(format!(
                    "Approval required: {} — {}",
                    tool, description
                )));
            }
            AgentEvent::ApprovalResolved { approved, .. } => {
                let line = if *approved {
                    "Approved.".to_string()
                } else {
                    "Denied.".to_string()
                };
                self.blocks.push(DisplayBlock::System(line));
            }
            AgentEvent::CostUpdated {
                input_tokens,
                output_tokens,
                estimated_cost_usd,
            } => {
                self.input_tokens = *input_tokens;
                self.output_tokens = *output_tokens;
                self.cost_usd = *estimated_cost_usd;
            }
            AgentEvent::Error { message } => {
                self.blocks.push(DisplayBlock::ErrorLine(message.clone()));
            }
            AgentEvent::Checkpoint { .. } => {}
            AgentEvent::ChildSessionSpawned {
                child_session_id,
                task,
                ..
            } => {
                let short = if child_session_id.len() > 8 {
                    &child_session_id[..8]
                } else {
                    child_session_id.as_str()
                };
                self.blocks.push(DisplayBlock::System(format!(
                    "Sub-agent {short}: {}",
                    truncate(task, 80)
                )));
            }
            AgentEvent::ChildSessionCompleted {
                child_session_id,
                status,
                ..
            } => {
                let short = if child_session_id.len() > 8 {
                    &child_session_id[..8]
                } else {
                    child_session_id.as_str()
                };
                self.blocks.push(DisplayBlock::System(format!(
                    "Sub-agent {short} done: {status}"
                )));
            }
            _ => {}
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        t.to_string()
    } else {
        format!("{}…", t.chars().take(max.saturating_sub(1)).collect::<String>())
    }
}
