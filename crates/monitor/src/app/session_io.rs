//! Session persistence helpers (disk + attach).

use super::types::{ChatEntry, ChatRole};
use crate::controller::LiveAttachController;
use nca_common::config::NcaConfig;
use nca_common::message::{Message, Role};
use nca_runtime::service::ServiceSessionInfo;
use nca_runtime::session_store::SessionStore;
use std::path::{Path, PathBuf};

pub(crate) fn attach_controller(info: &ServiceSessionInfo) -> Result<LiveAttachController, String> {
    let socket_path = info
        .socket_path
        .clone()
        .ok_or_else(|| "session did not expose a socket path".to_string())?;
    Ok(LiveAttachController::attach(socket_path))
}

pub(crate) fn load_session_metas(workspace_root: &Path, config: &NcaConfig) -> Vec<nca_common::session::SessionMeta> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let Ok(rt) = runtime else {
        return Vec::new();
    };
    let store = SessionStore::new(workspace_root.join(&config.session.history_dir));
    let mut sessions = Vec::new();
    if let Ok(ids) = rt.block_on(store.list()) {
        for id in ids {
            if let Ok(state) = rt.block_on(store.load(&id)) {
                sessions.push(state.meta);
            }
        }
    }
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    sessions
}

pub(crate) fn load_transcript(
    workspace_root: &Path,
    config: &NcaConfig,
    session_id: &str,
) -> Vec<ChatEntry> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let Ok(rt) = runtime else {
        return Vec::new();
    };
    let store = SessionStore::new(workspace_root.join(&config.session.history_dir));
    let Ok(state) = rt.block_on(store.load(session_id)) else {
        return Vec::new();
    };
    state
        .messages
        .iter()
        .filter_map(message_to_chat_entry)
        .collect()
}

pub(crate) fn load_session_state(
    workspace_root: &Path,
    config: &NcaConfig,
    session_id: &str,
) -> Option<nca_common::session::SessionState> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
    let store = SessionStore::new(workspace_root.join(&config.session.history_dir));
    runtime.block_on(store.load(session_id)).ok()
}

pub(crate) fn workspace_event_log_path(
    workspace_root: &Path,
    config: &NcaConfig,
    session_id: &str,
) -> PathBuf {
    workspace_root
        .join(&config.session.history_dir)
        .join(format!("{session_id}.events.jsonl"))
}

pub(crate) fn message_to_chat_entry(message: &Message) -> Option<ChatEntry> {
    if message.content.trim().is_empty() {
        return None;
    }
    match message.role {
        Role::User => Some(ChatEntry {
            role: ChatRole::User,
            title: "Developer".into(),
            content: message.content.clone(),
        }),
        Role::Assistant => Some(ChatEntry {
            role: ChatRole::Assistant,
            title: "Orchestrator".into(),
            content: message.content.clone(),
        }),
        Role::Tool => Some(ChatEntry {
            role: ChatRole::Tool,
            title: "System".into(),
            content: message.content.clone(),
        }),
        Role::System => None,
    }
}
