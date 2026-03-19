//! Desktop shell: orchestration, sessions, chat, git/worktrees.

mod git_worktree;
mod palette;
mod session_io;
mod types;
mod widgets;

use crate::workspaces::WorkspaceManager;
use eframe::egui;
use nca_common::config::NcaConfig;
use nca_common::event::{AgentCommand, AgentEvent};
use nca_common::orchestration::{
    AgentProfile, AgentProfileId, Company, CompanyId, DesktopMode, NewAgentProfile, NewCompany,
    NewProject, NewTodo, OrchestrationSnapshot, Project, ProjectId, RunLaunchContext, RunLink,
    Todo, TodoId, TodoPriority, TodoStatus,
};
use nca_common::session::{SessionMeta, SessionStatus};
use nca_runtime::service::{
    OrchestrationService, ServiceSessionInfo, ServiceSessionKind, ServiceSessionRequest,
};
use rfd::FileDialog;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use types::*;

pub struct DesktopApp {
    pub(crate) orchestration_service: OrchestrationService,
    pub(crate) orchestration: OrchestrationSnapshot,
    pub(crate) desktop_mode: DesktopMode,
    pub(crate) selected_company_id: Option<CompanyId>,
    pub(crate) selected_project_id: Option<ProjectId>,
    pub(crate) selected_todo_id: Option<TodoId>,
    pub(crate) selected_agent_id: Option<AgentProfileId>,
    pub(crate) selected_session_id: Option<String>,
    pub(crate) workspace_mgr: WorkspaceManager,
    pub(crate) view: View,
    pub(crate) settings_scope: SettingsScope,
    pub(crate) global_settings: NcaConfig,
    pub(crate) project_settings: Option<NcaConfig>,
    pub(crate) composer: ComposerState,
    pub(crate) project_sessions: Vec<SessionMeta>,
    pub(crate) active_session: Option<ActiveSession>,
    pub(crate) company_form: CompanyForm,
    pub(crate) project_form: ProjectForm,
    pub(crate) todo_form: TodoForm,
    pub(crate) agent_form: AgentForm,
    pub(crate) status_message: Option<(String, bool, Instant)>,
    /// Pending destructive git confirmation: (session_id, base_branch, delete_branch_after_remove).
    pub(crate) git_pending_remove: Option<(String, String, bool)>,
    pub(crate) git_pending_merge: Option<(String, String)>,
    pub(crate) git_selected_session_id: Option<String>,
    pub(crate) git_selected_file: Option<PathBuf>,
    pub(crate) git_diff_buffer: String,
}

impl DesktopApp {
    pub fn new() -> Self {
        let orchestration_service = OrchestrationService::default();
        let orchestration = orchestration_service.load_snapshot().unwrap_or_default();
        let mut workspace_mgr = WorkspaceManager::load();
        workspace_mgr.sort_by_recent();
        if !workspace_mgr.workspaces.is_empty() {
            workspace_mgr.select(Some(0));
        }

        let global_settings = NcaConfig::load_global_file().unwrap_or_default();
        let mut app = Self {
            orchestration_service,
            desktop_mode: orchestration.mode.mode,
            orchestration,
            selected_company_id: None,
            selected_project_id: None,
            selected_todo_id: None,
            selected_agent_id: None,
            selected_session_id: None,
            workspace_mgr,
            view: View::Dashboard,
            settings_scope: SettingsScope::Project,
            global_settings,
            project_settings: None,
            composer: ComposerState::default(),
            project_sessions: Vec::new(),
            active_session: None,
            company_form: CompanyForm::default(),
            project_form: ProjectForm::default(),
            todo_form: TodoForm::default(),
            agent_form: AgentForm::default(),
            status_message: None,
            git_pending_remove: None,
            git_pending_merge: None,
            git_selected_session_id: None,
            git_selected_file: None,
            git_diff_buffer: String::new(),
        };
        app.sync_orchestration_selection();
        app.reload_selected_workspace_data();
        app
    }

    fn reload_orchestration_data(&mut self) {
        match self.orchestration_service.load_snapshot() {
            Ok(snapshot) => {
                self.desktop_mode = snapshot.mode.mode;
                self.orchestration = snapshot;
                self.sync_orchestration_selection();
            }
            Err(error) => self.set_status(error, true),
        }
    }

    fn sync_orchestration_selection(&mut self) {
        if self.orchestration.companies.is_empty() {
            self.selected_company_id = None;
            self.selected_project_id = None;
            self.selected_todo_id = None;
            self.selected_agent_id = None;
            return;
        }

        if self
            .selected_company_id
            .as_ref()
            .is_none_or(|id| !self.orchestration.companies.iter().any(|c| &c.id == id))
        {
            self.selected_company_id = self.orchestration.companies.first().map(|c| c.id.clone());
        }

        let project_ids: Vec<_> = self
            .selected_company_id
            .as_ref()
            .map(|company_id| {
                self.orchestration
                    .projects
                    .iter()
                    .filter(|project| &project.company_id == company_id)
                    .map(|project| project.id.clone())
                    .collect()
            })
            .unwrap_or_default();

        if self
            .selected_project_id
            .as_ref()
            .is_none_or(|id| !project_ids.iter().any(|candidate| candidate == id))
        {
            self.selected_project_id = project_ids.first().cloned();
        }

        let todo_ids: Vec<_> = self
            .selected_project_id
            .as_ref()
            .map(|project_id| {
                self.orchestration
                    .todos
                    .iter()
                    .filter(|todo| &todo.project_id == project_id)
                    .map(|todo| todo.id.clone())
                    .collect()
            })
            .unwrap_or_default();

        if self
            .selected_todo_id
            .as_ref()
            .is_none_or(|id| !todo_ids.iter().any(|candidate| candidate == id))
        {
            self.selected_todo_id = todo_ids.first().cloned();
        }

        let agent_ids: Vec<_> = self
            .selected_project_id
            .as_ref()
            .map(|project_id| {
                self.orchestration
                    .agents
                    .iter()
                    .filter(|agent| agent.project_id.as_ref() == Some(project_id))
                    .map(|agent| agent.id.clone())
                    .collect()
            })
            .unwrap_or_default();

        if self
            .selected_agent_id
            .as_ref()
            .is_none_or(|id| !agent_ids.iter().any(|candidate| candidate == id))
        {
            self.selected_agent_id = agent_ids.first().cloned();
        }
    }

    fn selected_company(&self) -> Option<&Company> {
        let id = self.selected_company_id.as_ref()?;
        self.orchestration
            .companies
            .iter()
            .find(|company| &company.id == id)
    }

    fn selected_project(&self) -> Option<&Project> {
        let id = self.selected_project_id.as_ref()?;
        self.orchestration
            .projects
            .iter()
            .find(|project| &project.id == id)
    }

    fn selected_todo(&self) -> Option<&Todo> {
        let id = self.selected_todo_id.as_ref()?;
        self.orchestration.todos.iter().find(|todo| &todo.id == id)
    }

    fn company_projects(&self) -> Vec<&Project> {
        let Some(company_id) = self.selected_company_id.as_ref() else {
            return Vec::new();
        };
        self.orchestration
            .projects
            .iter()
            .filter(|project| &project.company_id == company_id)
            .collect()
    }

    fn project_todos(&self) -> Vec<&Todo> {
        let Some(project_id) = self.selected_project_id.as_ref() else {
            return Vec::new();
        };
        let mut todos: Vec<_> = self
            .orchestration
            .todos
            .iter()
            .filter(|todo| &todo.project_id == project_id)
            .collect();
        todos.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        todos
    }

    fn project_agents(&self) -> Vec<&AgentProfile> {
        let company_id = self.selected_company_id.as_ref();
        let project_id = self.selected_project_id.as_ref();
        let mut agents: Vec<_> = self
            .orchestration
            .agents
            .iter()
            .filter(|agent| {
                agent.project_id.as_ref() == project_id
                    || (agent.project_id.is_none() && agent.company_id.as_ref() == company_id)
            })
            .collect();
        agents.sort_by(|a, b| a.name.cmp(&b.name));
        agents
    }

    fn project_run_links(&self) -> Vec<&RunLink> {
        let todo_ids: std::collections::BTreeSet<_> = self
            .project_todos()
            .into_iter()
            .map(|todo| todo.id.clone())
            .collect();
        let mut runs: Vec<_> = self
            .orchestration
            .run_links
            .iter()
            .filter(|run| todo_ids.contains(&run.todo_id))
            .collect();
        runs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        runs
    }

    fn set_desktop_mode(&mut self, mode: DesktopMode) {
        if self.desktop_mode == mode {
            return;
        }
        match self.orchestration_service.save_mode(mode) {
            Ok(pref) => {
                self.desktop_mode = pref.mode;
                self.reload_orchestration_data();
            }
            Err(error) => self.set_status(error, true),
        }
    }

    fn create_company(&mut self) {
        if self.company_form.name.trim().is_empty() {
            self.set_status("Enter a company name first.", true);
            return;
        }
        let input = NewCompany {
            name: self.company_form.name.clone(),
            description: widgets::clean_optional_text(&self.company_form.description),
        };
        match self.orchestration_service.create_company(input) {
            Ok(company) => {
                self.company_form = CompanyForm::default();
                self.reload_orchestration_data();
                self.selected_company_id = Some(company.id);
                self.set_status("Company created.", false);
            }
            Err(error) => self.set_status(error, true),
        }
    }

    fn create_project(&mut self) {
        let Some(company_id) = self.selected_company_id.clone() else {
            self.set_status("Create or select a company first.", true);
            return;
        };
        if self.project_form.name.trim().is_empty() {
            self.set_status("Enter a project name first.", true);
            return;
        }
        let workspace_root =
            widgets::clean_optional_text(&self.project_form.workspace_root).map(PathBuf::from);
        let input = NewProject {
            company_id,
            name: self.project_form.name.clone(),
            slug: self.project_form.slug.clone(),
            description: widgets::clean_optional_text(&self.project_form.description),
            workspace_root,
        };
        match self.orchestration_service.create_project(input) {
            Ok(project) => {
                let task_lines: Vec<String> = self
                    .project_form
                    .initial_tasks
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect();
                self.project_form = ProjectForm::default();
                self.reload_orchestration_data();
                self.selected_project_id = Some(project.id.clone());
                for title in task_lines {
                    let input_todo = NewTodo {
                        project_id: project.id.clone(),
                        title,
                        description: None,
                        priority: TodoPriority::Medium,
                        acceptance_criteria: Vec::new(),
                    };
                    let _ = self.orchestration_service.create_todo(input_todo);
                }
                self.reload_orchestration_data();
                self.sync_orchestration_selection();
                self.reload_selected_workspace_data();
                let msg = if self.orchestration.todos.iter().any(|t| t.project_id == project.id) {
                    "Project created with starter tasks."
                } else {
                    "Project created."
                };
                self.set_status(msg, false);
            }
            Err(error) => self.set_status(error, true),
        }
    }

    fn create_todo(&mut self) {
        let Some(project_id) = self.selected_project_id.clone() else {
            self.set_status("Select a project before creating a todo.", true);
            return;
        };
        if self.todo_form.title.trim().is_empty() {
            self.set_status("Enter a todo title first.", true);
            return;
        }
        let acceptance_criteria = self
            .todo_form
            .acceptance_criteria
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();
        let input = NewTodo {
            project_id,
            title: self.todo_form.title.clone(),
            description: widgets::clean_optional_text(&self.todo_form.description),
            priority: self.todo_form.priority,
            acceptance_criteria,
        };
        match self.orchestration_service.create_todo(input) {
            Ok(todo) => {
                self.todo_form = TodoForm::default();
                self.reload_orchestration_data();
                self.selected_todo_id = Some(todo.id);
                self.set_status("Todo created.", false);
            }
            Err(error) => self.set_status(error, true),
        }
    }

    fn create_agent_profile(&mut self) {
        if self.agent_form.name.trim().is_empty() || self.agent_form.role.trim().is_empty() {
            self.set_status("Enter an agent name and role first.", true);
            return;
        }
        let input = NewAgentProfile {
            company_id: self.selected_company_id.clone(),
            project_id: self.selected_project_id.clone(),
            name: self.agent_form.name.clone(),
            role: self.agent_form.role.clone(),
            model: widgets::clean_optional_text(&self.agent_form.model),
            workspace_root: self
                .selected_project()
                .and_then(|project| project.workspace_root.clone()),
            prompt_hint: widgets::clean_optional_text(&self.agent_form.prompt_hint),
        };
        match self.orchestration_service.create_agent_profile(input) {
            Ok(agent) => {
                self.agent_form = AgentForm::default();
                self.reload_orchestration_data();
                self.selected_agent_id = Some(agent.id);
                self.set_status("Agent created.", false);
            }
            Err(error) => self.set_status(error, true),
        }
    }

    fn assign_selected_todo(&mut self, agent_id: Option<AgentProfileId>) {
        let Some(todo_id) = self.selected_todo_id.clone() else {
            return;
        };
        match self
            .orchestration_service
            .assign_todo(&todo_id, agent_id.as_ref())
        {
            Ok(()) => {
                self.reload_orchestration_data();
                self.set_status("Todo assignment updated.", false);
            }
            Err(error) => self.set_status(error, true),
        }
    }

    fn update_selected_todo_status(&mut self, status: TodoStatus) {
        let Some(todo_id) = self.selected_todo_id.clone() else {
            return;
        };
        match self
            .orchestration_service
            .update_todo_status(&todo_id, status)
        {
            Ok(()) => {
                self.reload_orchestration_data();
                self.set_status("Todo status updated.", false);
            }
            Err(error) => self.set_status(error, true),
        }
    }

    fn select_project_workspace_for_form(&mut self) {
        if let Some(path) = FileDialog::new().pick_folder() {
            self.project_form.workspace_root = path.display().to_string();
        }
    }

    fn launch_selected_todo(&mut self) {
        let Some(todo) = self.selected_todo().cloned() else {
            self.set_status("Select a todo to launch.", true);
            return;
        };
        let mut prompt = format!("Project task: {}\n\n", todo.title);
        if let Some(project) = self.selected_project() {
            prompt.push_str(&format!("Project: {}\n", project.name));
        }
        if let Some(description) = &todo.description {
            prompt.push_str(&format!("\nDescription:\n{}\n", description));
        }
        if !todo.acceptance_criteria.is_empty() {
            prompt.push_str("\nAcceptance criteria:\n");
            for item in &todo.acceptance_criteria {
                prompt.push_str(&format!("- {item}\n"));
            }
        }

        let launch_context = RunLaunchContext {
            todo_id: todo.id.clone(),
            agent_id: todo
                .assigned_agent_id
                .clone()
                .or_else(|| self.selected_agent_id.clone()),
        };
        self.composer.prompt = prompt.clone();
        self.start_session_from_prompt(prompt, Some(launch_context));
    }

    fn delete_session(&mut self, session_id: &str) {
        if let Some(workspace_root) = self.selected_workspace() {
            let config = self.effective_project_config();
            let sessions_dir = workspace_root.join(&config.session.history_dir);
            let _ = std::fs::remove_file(sessions_dir.join(format!("{}.json", session_id)));
            let _ = std::fs::remove_file(sessions_dir.join(format!("{}.events.jsonl", session_id)));
            let _ = std::fs::remove_file(sessions_dir.join(format!("{}.spawn.log", session_id)));
            self.reload_selected_workspace_data();
        }
    }

    fn set_status(&mut self, message: impl Into<String>, is_error: bool) {
        self.status_message = Some((message.into(), is_error, Instant::now()));
    }

    fn selected_workspace(&self) -> Option<PathBuf> {
        self.selected_project()
            .and_then(|project| project.workspace_root.clone())
            .or_else(|| self.workspace_mgr.selected_path().cloned())
    }

    fn effective_project_config(&self) -> NcaConfig {
        self.project_settings
            .clone()
            .or_else(|| {
                self.selected_workspace()
                    .and_then(|path| NcaConfig::load_for_workspace(&path).ok())
            })
            .unwrap_or_else(|| self.global_settings.clone())
    }

    fn reload_selected_workspace_data(&mut self) {
        let selected = self.selected_workspace();
        self.project_settings = selected
            .as_ref()
            .and_then(|path| NcaConfig::load_for_workspace(path).ok());
        self.project_sessions = selected
            .as_ref()
            .map(|path| session_io::load_session_metas(path, &self.effective_project_config()))
            .unwrap_or_default();
        let config = self.effective_project_config();
        if self.composer.model.is_empty() {
            self.composer.model = config.model.default_model;
        }
        self.composer.permission_mode = config.permissions.mode;
        self.sync_run_links_for_linked_sessions();
    }

    /// Persist git fields on orchestration run_links when session JSON has worktree/branch/parent.
    fn sync_run_links_for_linked_sessions(&mut self) {
        let link_ids: std::collections::HashSet<String> = self
            .orchestration
            .run_links
            .iter()
            .map(|r| r.session_id.clone())
            .collect();
        let mut any = false;
        for meta in &self.project_sessions {
            if !link_ids.contains(&meta.id) {
                continue;
            }
            if meta.worktree_path.is_none()
                && meta.branch.is_none()
                && meta.parent_session_id.is_none()
            {
                continue;
            }
            if let Ok(updated) = self.orchestration_service.update_run_link_git_fields(
                &meta.id,
                meta.worktree_path.as_ref(),
                meta.branch.as_deref(),
                meta.parent_session_id.as_deref(),
            ) {
                if updated {
                    any = true;
                }
            }
        }
        if any {
            self.reload_orchestration_data();
        }
    }

    pub(crate) fn open_child_session(&mut self, child_id: &str) {
        let ws = self
            .active_session
            .as_ref()
            .map(|s| s.info.workspace_root.clone())
            .or_else(|| self.selected_workspace());
        let Some(ws) = ws else {
            self.set_status("Pick a workspace first.", true);
            return;
        };
        let config = NcaConfig::load_for_workspace(&ws)
            .unwrap_or_else(|_| self.effective_project_config());
        if let Some(state) = session_io::load_session_state(&ws, &config, child_id) {
            self.resume_or_attach_session(state.meta);
        } else {
            self.set_status(format!("No saved session for {child_id}"), true);
        }
    }

    fn open_project_dialog(&mut self) {
        if let Some(path) = FileDialog::new().pick_folder() {
            self.workspace_mgr.add_workspace(path.clone());
            self.workspace_mgr.sort_by_recent();
            let selected_idx = self
                .workspace_mgr
                .workspaces
                .iter()
                .position(|w| w.path == path);
            self.workspace_mgr.select(selected_idx);
            self.reload_selected_workspace_data();
            self.view = View::Projects;
        }
    }

    fn start_new_session(&mut self) {
        let prompt = self.composer.prompt.clone();
        self.start_session_from_prompt(prompt, None);
    }

    fn start_session_from_prompt(
        &mut self,
        prompt: String,
        launch_context: Option<RunLaunchContext>,
    ) {
        let Some(workspace_root) = self.selected_workspace() else {
            self.set_status("Pick a project folder first.", true);
            return;
        };
        if prompt.trim().is_empty() {
            self.set_status("Enter a prompt before starting a chat.", true);
            return;
        }
        let mut config = self.effective_project_config();
        let model = if self.composer.model.trim().is_empty() {
            config.model.default_model.clone()
        } else {
            self.composer.model.trim().to_string()
        };
        config.model.default_model = model.clone();
        config.provider.minimax.model = model;
        config.permissions.mode = self.composer.permission_mode;

        match nca_runtime::service::spawn_service_session(ServiceSessionRequest {
            config,
            workspace_root: workspace_root.clone(),
            safe_mode: self.composer.safe_mode,
            initial_prompt: Some(prompt),
            orchestration_context: None,
            launch_context,
            kind: ServiceSessionKind::New { session_id: None },
        }) {
            Ok(handle) => {
                let info = handle.info().clone();
                match session_io::attach_controller(&info) {
                    Ok(controller) => {
                        let mut session =
                            ActiveSession::from_loaded(info, controller, Some(handle), Vec::new());
                        session.run_in_progress = true;
                        self.selected_session_id = Some(session.info.session_id.clone());
                        self.active_session = Some(session);
                        self.composer.prompt.clear();
                        self.view = View::Chat;
                        self.reload_orchestration_data();
                        self.reload_selected_workspace_data();
                    }
                    Err(e) => self.set_status(e, true),
                }
            }
            Err(e) => self.set_status(e, true),
        }
    }

    pub(crate) fn resume_or_attach_session(&mut self, meta: SessionMeta) {
        let transcript =
            session_io::load_transcript(&meta.workspace, &self.effective_project_config(), &meta.id);
        if meta.status == SessionStatus::Running {
            if let Some(socket_path) = meta.socket_path.clone() {
                let info = ServiceSessionInfo {
                    session_id: meta.id.clone(),
                    workspace_root: meta.workspace.clone(),
                    model: meta.model.clone(),
                    socket_path: Some(socket_path),
                    event_log_path: session_io::workspace_event_log_path(
                        &meta.workspace,
                        &self.effective_project_config(),
                        &meta.id,
                    ),
                };
                match session_io::attach_controller(&info) {
                    Ok(controller) => {
                        self.active_session = Some(ActiveSession::from_loaded(
                            info, controller, None, transcript,
                        ));
                        self.selected_session_id = Some(meta.id.clone());
                        self.view = View::Chat;
                    }
                    Err(e) => self.set_status(e, true),
                }
                return;
            }
        }
        match nca_runtime::service::spawn_service_session(ServiceSessionRequest {
            config: self.effective_project_config(),
            workspace_root: meta.workspace.clone(),
            safe_mode: false,
            initial_prompt: None,
            orchestration_context: None,
            launch_context: None,
            kind: ServiceSessionKind::Resume {
                session_id: meta.id.clone(),
            },
        }) {
            Ok(handle) => {
                let info = handle.info().clone();
                match session_io::attach_controller(&info) {
                    Ok(controller) => {
                        self.active_session = Some(ActiveSession::from_loaded(
                            info,
                            controller,
                            Some(handle),
                            transcript,
                        ));
                        self.selected_session_id = Some(meta.id.clone());
                        self.view = View::Chat;
                        self.reload_selected_workspace_data();
                    }
                    Err(e) => self.set_status(e, true),
                }
            }
            Err(e) => self.set_status(e, true),
        }
    }

    fn process_live_events(&mut self) {
        let mut refresh_sessions = false;
        let mut refresh_orchestration = false;
        {
            let Some(session) = self.active_session.as_mut() else {
                return;
            };
            for event in session.controller.drain() {
                match event {
                    AgentEvent::SessionStarted {
                        session_id, model, ..
                    } => {
                        session.info.session_id = session_id;
                        session.info.model = model;
                        session.ended = None;
                        session.last_error = None;
                    }
                    AgentEvent::MessageReceived { role, content } => match role.as_str() {
                        "user" => {
                            if !content.trim().is_empty() {
                                session.push_user(content);
                            }
                            session.run_in_progress = true;
                        }
                        "assistant" => {
                            session.push_assistant(content);
                            session.run_in_progress = false;
                        }
                        _ => {}
                    },
                    AgentEvent::TokensStreamed { delta } => {
                        session.streaming_assistant.push_str(&delta);
                    }
                    AgentEvent::ToolCallStarted { tool, input, .. } => {
                        session.push_tool(format!("[exec] {tool} {input}"));
                    }
                    AgentEvent::ToolCallCompleted { output, .. } => {
                        if output.success {
                            session.push_tool("[done] ok".into());
                        } else if let Some(e) = output.error {
                            session.push_error(e);
                        }
                    }
                    AgentEvent::CostUpdated {
                        input_tokens,
                        output_tokens,
                        estimated_cost_usd,
                    } => {
                        session.input_tokens = input_tokens;
                        session.output_tokens = output_tokens;
                        session.estimated_cost_usd = estimated_cost_usd;
                    }
                    AgentEvent::ApprovalRequested {
                        call_id,
                        tool,
                        description,
                    } => {
                        session.pending_approvals.push(PendingApproval {
                            call_id,
                            tool,
                            description,
                        });
                    }
                    AgentEvent::ApprovalResolved { call_id, approved } => {
                        session.pending_approvals.retain(|a| a.call_id != call_id);
                        if !approved {
                            session.push_error("Tool approval was denied.".into());
                        }
                    }
                    AgentEvent::SessionEnded { reason } => {
                        session.ended = Some(reason);
                        session.run_in_progress = false;
                        refresh_sessions = true;
                        refresh_orchestration = true;
                    }
                    AgentEvent::Error { message } => {
                        session.push_error(message);
                        session.run_in_progress = false;
                    }
                    AgentEvent::ChildSessionSpawned {
                        child_session_id, ..
                    } => {
                        if !session.child_session_ids.contains(&child_session_id) {
                            session.child_session_ids.push(child_session_id);
                        }
                        refresh_sessions = true;
                    }
                    AgentEvent::ChildSessionCompleted { .. } => {
                        refresh_sessions = true;
                    }
                    AgentEvent::Checkpoint { .. }
                    | AgentEvent::Response { .. }
                    | AgentEvent::TodoStatusChanged { .. }
                    | AgentEvent::TodoAssigned { .. }
                    | AgentEvent::RunLinked { .. }
                    | AgentEvent::DesktopModeChanged { .. } => {}
                }
            }
        }
        if refresh_sessions {
            self.reload_selected_workspace_data();
        }
        if refresh_orchestration {
            self.reload_orchestration_data();
        }
    }

    // -----------------------------------------------------------------------
    // Sidebar — dark, compact, matches dashboard1.html
    // -----------------------------------------------------------------------
    fn show_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("sidebar")
            .exact_width(250.0)
            .frame(
                egui::Frame::none()
                    .fill(palette::SIDEBAR)
                    .inner_margin(egui::Margin::same(0.0)),
            )
            .show(ctx, |ui| {
                ui.style_mut().visuals.widgets.noninteractive.bg_fill = palette::SIDEBAR;

                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.colored_label(
                        palette::WHITE,
                        egui::RichText::new("nca desktop").strong().size(16.0),
                    );
                });
                ui.add_space(10.0);

                egui::Frame::none()
                    .inner_margin(egui::Margin::symmetric(12.0, 0.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            let is_project = self.desktop_mode == DesktopMode::ProjectAi;
                            let is_company = self.desktop_mode == DesktopMode::CompanyAi;
                            if widgets::mode_pill(ui, is_project, "Project AI").clicked() {
                                self.set_desktop_mode(DesktopMode::ProjectAi);
                            }
                            if widgets::mode_pill(ui, is_company, "Company AI").clicked() {
                                self.set_desktop_mode(DesktopMode::CompanyAi);
                            }
                        });
                    });
                ui.add_space(14.0);
                widgets::draw_separator(ui);
                ui.add_space(8.0);
                let mut nav_items = vec![
                    (View::Dashboard, "Dashboard"),
                    (View::Projects, "Projects"),
                    (View::Todos, "Todos"),
                    (View::Agents, "Agents"),
                    (View::Chat, "Chat"),
                    (View::Git, "Git"),
                    (View::Settings, "Settings"),
                ];
                if self.desktop_mode == DesktopMode::ProjectAi {
                    nav_items.remove(0);
                }
                for (view, label) in nav_items {
                    if widgets::draw_nav_link(ui, self.view == view, label).clicked() {
                        self.view = view;
                    }
                }

                ui.add_space(16.0);
                widgets::draw_separator(ui);
                ui.add_space(8.0);
                egui::ScrollArea::vertical()
                    .max_height((ui.available_height() - 52.0).max(20.0))
                    .show(ui, |ui| {
                        widgets::section_label(ui, "COMPANIES");
                        if self.orchestration.companies.is_empty() {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);
                                ui.colored_label(palette::TEXT_DIM, "Create your first company.");
                            });
                        } else {
                            let companies = self.orchestration.companies.clone();
                            for company in companies {
                                if widgets::draw_entity_tile(
                                    ui,
                                    self.selected_company_id.as_ref() == Some(&company.id),
                                    &company.name,
                                    company.description.as_deref().unwrap_or(""),
                                )
                                .clicked()
                                {
                                    self.selected_company_id = Some(company.id.clone());
                                    self.sync_orchestration_selection();
                                    self.reload_selected_workspace_data();
                                }
                            }
                        }

                        ui.add_space(10.0);
                        widgets::section_label(ui, "PROJECTS");
                        if self.company_projects().is_empty() {
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);
                                ui.colored_label(palette::TEXT_DIM, "No projects in this company.");
                            });
                        } else {
                            let projects: Vec<_> =
                                self.company_projects().into_iter().cloned().collect();
                            for project in projects {
                                let subtitle = project
                                    .workspace_root
                                    .as_ref()
                                    .map(|path| widgets::truncate_path(&path.display().to_string(), 28))
                                    .unwrap_or_else(|| "no workspace linked".into());
                                if widgets::draw_entity_tile(
                                    ui,
                                    self.selected_project_id.as_ref() == Some(&project.id),
                                    &project.name,
                                    &subtitle,
                                )
                                .clicked()
                                {
                                    self.selected_project_id = Some(project.id.clone());
                                    self.sync_orchestration_selection();
                                    self.reload_selected_workspace_data();
                                }
                            }
                        }

                        ui.add_space(10.0);
                        widgets::section_label(ui, "AGENTS");
                        let agents: Vec<_> = self.project_agents().into_iter().cloned().collect();
                        if agents.is_empty() {
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);
                                ui.colored_label(palette::TEXT_DIM, "No agents yet.");
                            });
                        } else {
                            for agent in agents {
                                if widgets::draw_entity_tile(
                                    ui,
                                    self.selected_agent_id.as_ref() == Some(&agent.id),
                                    &agent.name,
                                    &agent.role,
                                )
                                .clicked()
                                {
                                    self.selected_agent_id = Some(agent.id.clone());
                                }
                            }
                        }
                    });

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    widgets::draw_separator(ui);
                    ui.add_space(8.0);
                    let btn = egui::Button::new(
                        egui::RichText::new("Open Folder")
                            .size(12.0)
                            .color(palette::TEXT),
                    )
                    .fill(palette::CARD)
                    .stroke(egui::Stroke::new(1.0, palette::BORDER))
                    .rounding(6.0)
                    .min_size(egui::vec2(220.0, 34.0));
                    if ui.add(btn).clicked() {
                        self.open_project_dialog();
                    }
                    ui.add_space(4.0);
                });
            });
    }

    // -----------------------------------------------------------------------
    // Top header bar
    // -----------------------------------------------------------------------
    fn show_header(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header")
            .exact_height(44.0)
            .frame(
                egui::Frame::none()
                    .fill(palette::BG)
                    .inner_margin(egui::Margin::symmetric(20.0, 0.0))
                    .stroke(egui::Stroke::new(1.0, palette::BORDER)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let mut crumbs: Vec<(&str, egui::Color32)> = Vec::new();
                    if self.desktop_mode == DesktopMode::CompanyAi {
                        if let Some(company) = self.selected_company() {
                            crumbs.push((&company.name, palette::WHITE));
                        }
                    }
                    if let Some(project) = self.selected_project() {
                        crumbs.push((&project.name, palette::TEXT));
                    }
                    let view_name = match self.view {
                        View::Dashboard => "Dashboard",
                        View::Projects => "Projects",
                        View::Todos => "Todos",
                        View::Agents => "Agents",
                        View::Chat => "Chat",
                        View::Git => "Git",
                        View::Settings => "Settings",
                    };
                    crumbs.push((view_name, palette::TEXT_DIM));

                    for (i, (label, color)) in crumbs.iter().enumerate() {
                        if i > 0 {
                            ui.add_space(4.0);
                            ui.colored_label(
                                palette::TEXT_DIM,
                                egui::RichText::new("/").size(12.0),
                            );
                            ui.add_space(4.0);
                        }
                        ui.colored_label(*color, egui::RichText::new(*label).size(13.0));
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Status indicator
                        if self
                            .active_session
                            .as_ref()
                            .map_or(false, |s| s.run_in_progress)
                        {
                            let dot = egui::RichText::new("●").size(10.0).color(palette::SUCCESS);
                            ui.label(dot);
                            ui.colored_label(
                                palette::TEXT_DIM,
                                egui::RichText::new("Agent Running").size(11.0),
                            );
                        } else {
                            let dot = egui::RichText::new("●").size(10.0).color(palette::TEXT_DIM);
                            ui.label(dot);
                            ui.colored_label(
                                palette::TEXT_DIM,
                                egui::RichText::new("Idle").size(11.0),
                            );
                        }

                        // Status toast
                        if let Some((msg, is_err, at)) = &self.status_message {
                            if at.elapsed() < Duration::from_secs(5) {
                                let c = if *is_err {
                                    palette::ERROR
                                } else {
                                    palette::SUCCESS
                                };
                                ui.add_space(16.0);
                                ui.colored_label(c, egui::RichText::new(msg).size(11.0));
                            }
                        }
                    });
                });
            });
    }

    // -----------------------------------------------------------------------
    // Projects view — create project (metadata + task list), list, then agent launcher
    // -----------------------------------------------------------------------
    fn show_projects_view(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(palette::BG)
                    .inner_margin(egui::Margin::symmetric(24.0, 0.0)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        let max_w = 820.0_f32.min((ui.available_width() - 24.0).max(200.0));
                        ui.set_max_width(max_w);
                        ui.add_space(24.0);

                        if self.selected_company_id.is_none() {
                            ui.colored_label(
                                palette::TEXT_DIM,
                                egui::RichText::new(
                                    "Select a company in the sidebar (or create one on Dashboard) before adding projects.",
                                )
                                .size(14.0),
                            );
                            return;
                        }

                        let company_name = self
                            .orchestration
                            .companies
                            .iter()
                            .find(|c| Some(&c.id) == self.selected_company_id.as_ref())
                            .map(|c| c.name.as_str())
                            .unwrap_or("Company");
                        ui.horizontal(|ui| {
                            ui.colored_label(
                                palette::TEXT_DIM,
                                egui::RichText::new(company_name).size(13.0),
                            );
                            ui.colored_label(
                                palette::TEXT_DIM,
                                egui::RichText::new(" / ").size(13.0),
                            );
                            ui.colored_label(
                                palette::WHITE,
                                egui::RichText::new("Projects").size(13.0).strong(),
                            );
                        });
                        ui.add_space(16.0);

                        widgets::panel_card(ui, "Create project", |ui| {
                            ui.colored_label(
                                palette::TEXT_DIM,
                                egui::RichText::new("Name, description, optional workspace path, and starter tasks (one per line → todos).")
                                    .size(11.0),
                            );
                            ui.add_space(10.0);
                            ui.add(
                                egui::TextEdit::singleline(&mut self.project_form.name)
                                    .hint_text("Project name"),
                            );
                            ui.add_space(8.0);
                            ui.add(
                                egui::TextEdit::singleline(&mut self.project_form.slug)
                                    .hint_text("Slug"),
                            );
                            ui.add_space(8.0);
                            ui.add(
                                egui::TextEdit::multiline(&mut self.project_form.description)
                                    .desired_rows(3)
                                    .hint_text("Description"),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new("Starter task list")
                                    .size(11.0)
                                    .color(palette::TEXT_DIM),
                            );
                            ui.add(
                                egui::TextEdit::multiline(&mut self.project_form.initial_tasks)
                                    .desired_rows(5)
                                    .hint_text("One task per line (creates todos after save)"),
                            );
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.project_form.workspace_root)
                                        .desired_width(220.0)
                                        .hint_text("/path/to/repo (optional)"),
                                );
                                if ui.button("Browse").clicked() {
                                    self.select_project_workspace_for_form();
                                }
                            });
                            ui.add_space(10.0);
                            if ui.button("Create project").clicked() {
                                self.create_project();
                            }
                        });

                        ui.add_space(24.0);
                        ui.colored_label(
                            palette::WHITE,
                            egui::RichText::new("Projects in this company").size(16.0).strong(),
                        );
                        ui.add_space(10.0);

                        let projects: Vec<_> =
                            self.company_projects().into_iter().cloned().collect();
                        if projects.is_empty() {
                            ui.colored_label(
                                palette::TEXT_DIM,
                                "No projects yet — use the form above.",
                            );
                        } else {
                            for project in projects {
                                let subtitle: String = project
                                    .description
                                    .as_deref()
                                    .filter(|s| !s.is_empty())
                                    .map(|s| s.to_string())
                                    .or_else(|| {
                                        project.workspace_root.as_ref().map(|p| {
                                            widgets::truncate_path(&p.display().to_string(), 40)
                                        })
                                    })
                                    .unwrap_or_else(|| "—".to_string());
                                if widgets::draw_entity_tile(
                                    ui,
                                    self.selected_project_id.as_ref() == Some(&project.id),
                                    &project.name,
                                    subtitle.as_str(),
                                )
                                .clicked()
                                {
                                    self.selected_project_id = Some(project.id.clone());
                                    self.sync_orchestration_selection();
                                    self.reload_selected_workspace_data();
                                }
                            }
                        }

                        ui.add_space(28.0);
                        widgets::draw_separator(ui);
                        ui.add_space(16.0);
                        ui.colored_label(
                            palette::WHITE,
                            egui::RichText::new("Launch agents").size(16.0).strong(),
                        );
                        ui.colored_label(
                            palette::TEXT_DIM,
                            egui::RichText::new(
                                "Uses the selected project’s workspace (or Open Folder). Ongoing chats: Chat view.",
                            )
                            .size(11.0),
                        );
                        ui.add_space(12.0);

                        if self.selected_workspace().is_none() {
                            ui.colored_label(
                                palette::TEXT_DIM,
                                "Link a workspace path to this project, or use Open Folder in the title bar, to launch agents and list sessions here.",
                            );
                        } else {
                            self.show_agent_launch_panel(ui);
                        }

                        ui.add_space(32.0);
                    });
            });
    }

    fn show_agent_launch_panel(&mut self, ui: &mut egui::Ui) {
        ui.colored_label(
            palette::WHITE,
            egui::RichText::new("Start a new chat").size(15.0).strong(),
        );
        ui.colored_label(
            palette::TEXT_DIM,
            egui::RichText::new(
                "Initialize an AI agent to perform tasks within your project directory.",
            )
            .size(12.0),
        );
        ui.add_space(12.0);

        egui::Frame::none()
            .fill(palette::CARD)
            .rounding(12.0)
            .stroke(egui::Stroke::new(1.0, palette::BORDER))
            .inner_margin(egui::Margin::same(0.0))
            .show(ui, |ui| {
                ui.add_space(4.0);
                egui::Frame::none()
                    .inner_margin(egui::Margin::symmetric(20.0, 16.0))
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.composer.prompt)
                                .font(egui::FontId::monospace(13.0))
                                .desired_rows(4)
                                .desired_width(f32::INFINITY)
                                .hint_text("Describe the task you want the agent to work on..."),
                        );
                    });

                widgets::draw_separator(ui);
                egui::Frame::none()
                    .fill(egui::Color32::from_rgba_premultiplied(0, 0, 0, 50))
                    .inner_margin(egui::Margin::symmetric(20.0, 14.0))
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.vertical(|ui| {
                                ui.colored_label(
                                    palette::TEXT_DIM,
                                    egui::RichText::new("MODEL").size(9.0).strong(),
                                );
                                ui.add_space(2.0);
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.composer.model)
                                        .desired_width(180.0)
                                        .hint_text("MiniMax-M2.5"),
                                );
                            });
                            ui.add_space(24.0);

                            ui.vertical(|ui| {
                                ui.colored_label(
                                    palette::TEXT_DIM,
                                    egui::RichText::new("PERMISSION MODE")
                                        .size(9.0)
                                        .strong(),
                                );
                                ui.add_space(2.0);
                                widgets::permission_mode_combo(ui, &mut self.composer.permission_mode);
                            });
                            ui.add_space(24.0);

                            ui.vertical(|ui| {
                                ui.add_space(14.0);
                                ui.checkbox(
                                    &mut self.composer.safe_mode,
                                    egui::RichText::new("Safe Mode (Read-only)")
                                        .size(11.0)
                                        .color(palette::TEXT_DIM),
                                );
                            });

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let btn = egui::Button::new(
                                        egui::RichText::new("Launch Agent")
                                            .size(13.0)
                                            .strong()
                                            .color(palette::WHITE),
                                    )
                                    .fill(palette::ACCENT)
                                    .rounding(8.0)
                                    .min_size(egui::vec2(130.0, 36.0));
                                    if ui.add(btn).clicked() {
                                        self.start_new_session();
                                    }
                                },
                            );
                        });
                    });
            });

        ui.add_space(24.0);
        ui.horizontal(|ui| {
            ui.colored_label(
                palette::WHITE,
                egui::RichText::new("Recent sessions").size(15.0).strong(),
            );
        });
        ui.add_space(12.0);

        if self.project_sessions.is_empty() {
            ui.colored_label(
                palette::TEXT_DIM,
                "No saved sessions for this project yet.",
            );
        } else {
            let sessions = self.project_sessions.clone();
            let mut delete_id = None;

            for meta in &sessions {
                let is_running = meta.status == SessionStatus::Running;
                let border = if is_running {
                    palette::ACCENT
                } else {
                    palette::BORDER
                };

                egui::Frame::none()
                    .fill(palette::CARD)
                    .rounding(12.0)
                    .stroke(egui::Stroke::new(1.0, border))
                    .inner_margin(egui::Margin::symmetric(20.0, 16.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if is_running {
                                let rect = egui::Rect::from_min_size(
                                    ui.cursor().left_top() + egui::vec2(-20.0, -16.0),
                                    egui::vec2(3.0, ui.available_height() + 32.0),
                                );
                                ui.painter()
                                    .rect_filled(rect, 2.0, palette::ACCENT);
                            }

                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.colored_label(
                                        if is_running {
                                            palette::WHITE
                                        } else {
                                            palette::TEXT_DIM
                                        },
                                        egui::RichText::new(&meta.id)
                                            .monospace()
                                            .size(12.0)
                                            .strong(),
                                    );
                                    ui.add_space(8.0);
                                    let (badge_bg, badge_text, badge_label) =
                                        widgets::session_badge(&meta.status);
                                    egui::Frame::none()
                                        .fill(badge_bg)
                                        .rounding(4.0)
                                        .inner_margin(egui::Margin::symmetric(6.0, 2.0))
                                        .show(ui, |ui| {
                                            ui.colored_label(
                                                badge_text,
                                                egui::RichText::new(badge_label)
                                                    .size(9.0)
                                                    .strong(),
                                            );
                                        });
                                });
                                ui.add_space(4.0);
                                ui.colored_label(
                                    palette::TEXT_DIM,
                                    egui::RichText::new(format!(
                                        "Updated {}  ·  {}",
                                        widgets::format_time(&meta.updated_at),
                                        meta.model,
                                    ))
                                    .size(11.0),
                                );
                            });

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let del_btn = egui::Button::new(
                                        egui::RichText::new("Delete")
                                            .size(11.0)
                                            .color(palette::ERROR),
                                    )
                                    .fill(egui::Color32::TRANSPARENT)
                                    .stroke(egui::Stroke::NONE)
                                    .rounding(4.0);
                                    if ui.add(del_btn).clicked() {
                                        delete_id = Some(meta.id.clone());
                                    }

                                    let (label, fill) = if is_running {
                                        ("Open Running Chat", palette::ACCENT)
                                    } else {
                                        ("Resume in Desktop", palette::CARD)
                                    };
                                    let stroke = if is_running {
                                        egui::Stroke::NONE
                                    } else {
                                        egui::Stroke::new(1.0, palette::BORDER)
                                    };
                                    let action_btn = egui::Button::new(
                                        egui::RichText::new(label)
                                            .size(11.0)
                                            .strong()
                                            .color(palette::WHITE),
                                    )
                                    .fill(fill)
                                    .stroke(stroke)
                                    .rounding(6.0);
                                    if ui.add(action_btn).clicked() {
                                        self.resume_or_attach_session(meta.clone());
                                    }
                                },
                            );
                        });
                    });
                ui.add_space(8.0);
            }

            if let Some(id) = delete_id {
                self.delete_session(&id);
            }
        }
    }

    fn show_dashboard_view(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(palette::BG)
                    .inner_margin(egui::Margin::symmetric(24.0, 0.0)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        ui.add_space(24.0);
                        ui.horizontal_wrapped(|ui| {
                            widgets::stat_card(ui, "Projects", &self.company_projects().len().to_string());
                            widgets::stat_card(ui, "Open Todos", &self.project_todos().len().to_string());
                            widgets::stat_card(ui, "Agents", &self.project_agents().len().to_string());
                            widgets::stat_card(ui, "Runs", &self.project_run_links().len().to_string());
                        });
                        ui.add_space(18.0);

                        ui.columns(2, |columns| {
                            columns[0].add_space(4.0);
                            widgets::panel_card(&mut columns[0], "Create Company", |ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.company_form.name)
                                        .hint_text("Company name"),
                                );
                                ui.add_space(8.0);
                                ui.add(
                                    egui::TextEdit::multiline(&mut self.company_form.description)
                                        .desired_rows(3)
                                        .hint_text("Description"),
                                );
                                ui.add_space(8.0);
                                if ui.button("Create Company").clicked() {
                                    self.create_company();
                                }
                            });

                            widgets::panel_card(&mut columns[0], "Create Project", |ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.project_form.name)
                                        .hint_text("Project name"),
                                );
                                ui.add_space(8.0);
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.project_form.slug)
                                        .hint_text("Slug"),
                                );
                                ui.add_space(8.0);
                                ui.add(
                                    egui::TextEdit::multiline(&mut self.project_form.description)
                                        .desired_rows(3)
                                        .hint_text("Description"),
                                );
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new("Starter task list")
                                        .size(11.0)
                                        .color(palette::TEXT_DIM),
                                );
                                ui.add(
                                    egui::TextEdit::multiline(&mut self.project_form.initial_tasks)
                                        .desired_rows(4)
                                        .hint_text("One task per line (creates todos after save)"),
                                );
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    ui.add(
                                        egui::TextEdit::singleline(
                                            &mut self.project_form.workspace_root,
                                        )
                                        .desired_width(220.0)
                                        .hint_text("/path/to/repo"),
                                    );
                                    if ui.button("Browse").clicked() {
                                        self.select_project_workspace_for_form();
                                    }
                                });
                                ui.add_space(8.0);
                                if ui.button("Create Project").clicked() {
                                    self.create_project();
                                }
                            });
                        });

                        ui.add_space(16.0);
                        ui.columns(2, |columns| {
                            widgets::panel_card(&mut columns[0], "Create Todo", |ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.todo_form.title)
                                        .hint_text("Todo title"),
                                );
                                ui.add_space(8.0);
                                ui.add(
                                    egui::TextEdit::multiline(&mut self.todo_form.description)
                                        .desired_rows(3)
                                        .hint_text("Description"),
                                );
                                ui.add_space(8.0);
                                ui.label(egui::RichText::new("Acceptance Criteria").size(11.0));
                                ui.add(
                                    egui::TextEdit::multiline(
                                        &mut self.todo_form.acceptance_criteria,
                                    )
                                    .desired_rows(3)
                                    .hint_text("One line per acceptance criterion"),
                                );
                                ui.add_space(8.0);
                                widgets::todo_priority_combo(ui, &mut self.todo_form.priority);
                                ui.add_space(8.0);
                                if ui.button("Create Todo").clicked() {
                                    self.create_todo();
                                }
                            });

                            widgets::panel_card(&mut columns[1], "Create Agent", |ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.agent_form.name)
                                        .hint_text("Agent name"),
                                );
                                ui.add_space(8.0);
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.agent_form.role)
                                        .hint_text("Role"),
                                );
                                ui.add_space(8.0);
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.agent_form.model)
                                        .hint_text("MiniMax-M2.5"),
                                );
                                ui.add_space(8.0);
                                ui.add(
                                    egui::TextEdit::multiline(&mut self.agent_form.prompt_hint)
                                        .desired_rows(3)
                                        .hint_text("Prompt or operating hint"),
                                );
                                ui.add_space(8.0);
                                if ui.button("Create Agent").clicked() {
                                    self.create_agent_profile();
                                }
                            });
                        });

                        ui.add_space(20.0);
                        ui.colored_label(
                            palette::WHITE,
                            egui::RichText::new("Recent Linked Runs")
                                .size(16.0)
                                .strong(),
                        );
                        ui.add_space(10.0);
                        let runs: Vec<_> = self.project_run_links().into_iter().cloned().collect();
                        if runs.is_empty() {
                            ui.colored_label(
                                palette::TEXT_DIM,
                                "No linked runs yet. Launch a selected todo to create one.",
                            );
                        } else {
                            let mut open_chat = None;
                            let mut open_child: Option<(String, PathBuf)> = None;
                            let cfg = self.effective_project_config();
                            for run in runs {
                                let todo_title = self
                                    .orchestration
                                    .todos
                                    .iter()
                                    .find(|todo| todo.id == run.todo_id)
                                    .map(|todo| todo.title.clone())
                                    .unwrap_or_else(|| "Unknown todo".into());
                                let agent_label = run
                                    .agent_id
                                    .as_ref()
                                    .and_then(|id| {
                                        self.orchestration
                                            .agents
                                            .iter()
                                            .find(|agent| &agent.id == id)
                                            .map(|agent| agent.name.clone())
                                    })
                                    .unwrap_or_else(|| "Unassigned".into());
                                widgets::panel_card(
                                    ui,
                                    &format!("{todo_title} · {}", run.session_id),
                                    |ui| {
                                        ui.colored_label(
                                            palette::TEXT_DIM,
                                            format!(
                                                "{} · {}",
                                                agent_label,
                                                widgets::format_time(&run.updated_at)
                                            ),
                                        );
                                        if let Some(branch) = &run.branch {
                                            ui.colored_label(
                                                palette::TEXT_DIM,
                                                format!("branch: {branch}"),
                                            );
                                        }
                                        if let Some(worktree) = &run.worktree_path {
                                            ui.colored_label(
                                                palette::TEXT_DIM,
                                                widgets::truncate_path(&worktree.display().to_string(), 60),
                                            );
                                        }
                                        ui.add_space(6.0);
                                        if ui.button("Open Chat").clicked() {
                                            open_chat = Some(run.session_id.clone());
                                        }
                                        let parent_meta = self
                                            .project_sessions
                                            .iter()
                                            .find(|m| m.id == run.session_id)
                                            .cloned()
                                            .or_else(|| {
                                                session_io::load_session_state(
                                                    &run.workspace_root,
                                                    &cfg,
                                                    &run.session_id,
                                                )
                                                .map(|s| s.meta)
                                            });
                                        if let Some(pm) = parent_meta {
                                            if !pm.child_session_ids.is_empty() {
                                                ui.add_space(8.0);
                                                ui.label(
                                                    egui::RichText::new("Child sessions")
                                                        .size(11.0)
                                                        .strong(),
                                                );
                                                for cid in &pm.child_session_ids {
                                                    let child_label =
                                                        session_io::load_session_state(
                                                            &run.workspace_root,
                                                            &cfg,
                                                            cid,
                                                        )
                                                        .map(|s| {
                                                            let wt = s
                                                                .meta
                                                                .worktree_path
                                                                .as_ref()
                                                                .map(|p| {
                                                                    widgets::truncate_path(
                                                                        &p.display().to_string(),
                                                                        36,
                                                                    )
                                                                })
                                                                .unwrap_or_else(|| "main tree".into());
                                                            let br = s
                                                                .meta
                                                                .branch
                                                                .clone()
                                                                .unwrap_or_else(|| "—".into());
                                                            format!("{cid} · {br} · {wt}")
                                                        })
                                                        .unwrap_or_else(|| cid.clone());
                                                    ui.horizontal(|ui| {
                                                        ui.colored_label(
                                                            palette::TEXT_DIM,
                                                            &child_label,
                                                        );
                                                        if ui.small_button("Open").clicked() {
                                                            open_child = Some((
                                                                cid.clone(),
                                                                run.workspace_root.clone(),
                                                            ));
                                                        }
                                                    });
                                                }
                                            }
                                        }
                                    },
                                );
                            }
                            if let Some(session_id) = open_chat {
                                if let Some(meta) = self
                                    .project_sessions
                                    .iter()
                                    .find(|meta| meta.id == session_id)
                                    .cloned()
                                {
                                    self.resume_or_attach_session(meta);
                                }
                            }
                            if let Some((child_id, ws)) = open_child {
                                let config = NcaConfig::load_for_workspace(&ws)
                                    .unwrap_or_else(|_| self.effective_project_config());
                                if let Some(state) =
                                    session_io::load_session_state(&ws, &config, &child_id)
                                {
                                    self.resume_or_attach_session(state.meta);
                                }
                            }
                        }
                        ui.add_space(20.0);
                    });
            });
    }

    fn show_todos_view(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("todo_detail")
            .resizable(true)
            .default_width(320.0)
            .frame(
                egui::Frame::none()
                    .fill(palette::CARD)
                    .inner_margin(egui::Margin::same(16.0))
                    .stroke(egui::Stroke::new(1.0, palette::BORDER)),
            )
            .show(ctx, |ui| {
                ui.colored_label(
                    palette::WHITE,
                    egui::RichText::new("Todo Detail").size(15.0).strong(),
                );
                ui.add_space(12.0);
                let todo = self.selected_todo().cloned();
                if let Some(todo) = todo {
                    ui.label(egui::RichText::new(&todo.title).size(14.0).strong());
                    if let Some(desc) = &todo.description {
                        ui.add_space(8.0);
                        ui.colored_label(palette::TEXT_DIM, desc);
                    }
                    ui.add_space(10.0);
                    let mut status = todo.status;
                    widgets::todo_status_combo(ui, &mut status);
                    if status != todo.status && ui.button("Save Status").clicked() {
                        self.update_selected_todo_status(status);
                    }
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Assign Agent").size(11.0).strong());
                    for agent in self
                        .project_agents()
                        .into_iter()
                        .cloned()
                        .collect::<Vec<_>>()
                    {
                        let assigned = todo.assigned_agent_id.as_ref() == Some(&agent.id);
                        if ui
                            .selectable_label(assigned, format!("{} · {}", agent.name, agent.role))
                            .clicked()
                        {
                            self.assign_selected_todo(Some(agent.id.clone()));
                        }
                    }
                    if ui.button("Clear Assignment").clicked() {
                        self.assign_selected_todo(None);
                    }
                    ui.add_space(12.0);
                    if ui
                        .add(
                            egui::Button::new("Launch Run")
                                .fill(palette::ACCENT)
                                .min_size(egui::vec2(120.0, 32.0)),
                        )
                        .clicked()
                    {
                        self.launch_selected_todo();
                    }
                    if !todo.acceptance_criteria.is_empty() {
                        ui.add_space(12.0);
                        ui.label(egui::RichText::new("Acceptance").size(11.0).strong());
                        for item in &todo.acceptance_criteria {
                            ui.colored_label(palette::TEXT_DIM, format!("- {item}"));
                        }
                    }
                } else {
                    ui.colored_label(palette::TEXT_DIM, "Select a todo to inspect.");
                }
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(palette::BG)
                    .inner_margin(egui::Margin::same(16.0)),
            )
            .show(ctx, |ui| {
                let todos: Vec<_> = self.project_todos().into_iter().cloned().collect();
                if todos.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.colored_label(palette::TEXT_DIM, "No todos for this project yet.");
                    });
                    return;
                }

                let columns = [
                    TodoStatus::Backlog,
                    TodoStatus::InProgress,
                    TodoStatus::InReview,
                    TodoStatus::Done,
                ];
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        for status in columns {
                            let items: Vec<_> = todos
                                .iter()
                                .filter(|todo| todo.status == status)
                                .cloned()
                                .collect();
                            widgets::panel_card(ui, widgets::todo_status_label(status), |ui| {
                                ui.set_width(250.0);
                                if items.is_empty() {
                                    ui.colored_label(palette::TEXT_DIM, "No items");
                                } else {
                                    for todo in &items {
                                        let selected =
                                            self.selected_todo_id.as_ref() == Some(&todo.id);
                                        if widgets::draw_entity_tile(
                                            ui,
                                            selected,
                                            &todo.title,
                                            todo.description.as_deref().unwrap_or(""),
                                        )
                                        .clicked()
                                        {
                                            self.selected_todo_id = Some(todo.id.clone());
                                        }
                                    }
                                }
                            });
                        }
                    });
                });
            });
    }

    fn show_agents_view(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(palette::BG)
                    .inner_margin(egui::Margin::same(16.0)),
            )
            .show(ctx, |ui| {
                ui.colored_label(
                    palette::WHITE,
                    egui::RichText::new("Agents").size(18.0).strong(),
                );
                ui.add_space(12.0);
                let agents: Vec<_> = self.project_agents().into_iter().cloned().collect();
                if agents.is_empty() {
                    ui.colored_label(palette::TEXT_DIM, "No agents for this scope yet.");
                } else {
                    for agent in agents {
                        let assigned_count = self
                            .project_todos()
                            .into_iter()
                            .filter(|todo| todo.assigned_agent_id.as_ref() == Some(&agent.id))
                            .count();
                        widgets::panel_card(ui, &agent.name, |ui| {
                            ui.colored_label(
                                palette::TEXT_DIM,
                                format!(
                                    "{} · {}",
                                    agent.role,
                                    agent
                                        .model
                                        .clone()
                                        .unwrap_or_else(|| "default model".into())
                                ),
                            );
                            ui.colored_label(
                                palette::TEXT_DIM,
                                format!("{assigned_count} assigned todos"),
                            );
                            if ui.button("Select Agent").clicked() {
                                self.selected_agent_id = Some(agent.id.clone());
                                self.view = View::Todos;
                            }
                        });
                    }
                }
            });
    }

    // -----------------------------------------------------------------------
    // Chat view — matches dashboar-detail.html
    // -----------------------------------------------------------------------
    fn show_chat_view(&mut self, ctx: &egui::Context) {
        let has_session = self.active_session.is_some() || self.selected_session_id.is_some();
        if has_session {
        egui::SidePanel::right("chat_run_detail")
            .default_width(280.0)
            .resizable(true)
            .frame(
                egui::Frame::none()
                    .fill(palette::CARD)
                    .inner_margin(egui::Margin::same(16.0))
                    .stroke(egui::Stroke::new(1.0, palette::BORDER)),
            )
            .show(ctx, |ui| {
                ui.colored_label(
                    palette::WHITE,
                    egui::RichText::new("Run Detail").size(14.0).strong(),
                );
                ui.add_space(12.0);
                let active_session_id = self
                    .active_session
                    .as_ref()
                    .map(|session| session.info.session_id.clone())
                    .or_else(|| self.selected_session_id.clone());
                if let Some(session_id) = active_session_id {
                    ui.colored_label(
                        palette::TEXT,
                        egui::RichText::new(&session_id).monospace().size(11.0),
                    );
                    if let Some(run) = self
                        .orchestration
                        .run_links
                        .iter()
                        .find(|run| run.session_id == session_id)
                    {
                        if let Some(todo) = self
                            .orchestration
                            .todos
                            .iter()
                            .find(|todo| todo.id == run.todo_id)
                        {
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new("Todo").size(11.0).strong());
                            ui.colored_label(palette::TEXT_DIM, &todo.title);
                        }
                        if let Some(agent_id) = &run.agent_id {
                            if let Some(agent) = self
                                .orchestration
                                .agents
                                .iter()
                                .find(|agent| &agent.id == agent_id)
                            {
                                ui.add_space(8.0);
                                ui.label(egui::RichText::new("Agent").size(11.0).strong());
                                ui.colored_label(palette::TEXT_DIM, &agent.name);
                            }
                        }
                        if let Some(branch) = &run.branch {
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new("Branch").size(11.0).strong());
                            ui.colored_label(palette::TEXT_DIM, branch);
                        }
                    }

                    if let Some(active) = self.active_session.as_ref() {
                        ui.add_space(12.0);
                        ui.label(egui::RichText::new("Usage").size(11.0).strong());
                        ui.colored_label(
                            palette::TEXT_DIM,
                            format!(
                                "{} in · {} out · ${:.4}",
                                active.input_tokens,
                                active.output_tokens,
                                active.estimated_cost_usd
                            ),
                        );
                        if let Some(workspace) = active.info.workspace_root.to_str() {
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new("Workspace").size(11.0).strong());
                            ui.colored_label(palette::TEXT_DIM, widgets::truncate_path(workspace, 42));
                        }
                        if !active.child_session_ids.is_empty() {
                            ui.add_space(12.0);
                            ui.label(egui::RichText::new("Child Sessions").size(11.0).strong());
                            for child in active.child_session_ids.clone() {
                                ui.horizontal(|ui| {
                                    ui.colored_label(palette::TEXT_DIM, &child);
                                    if ui.small_button("Open").clicked() {
                                        self.open_child_session(&child);
                                    }
                                });
                            }
                        }
                    }

                    if let Some(workspace_root) = self.selected_workspace() {
                        if let Some(state) = session_io::load_session_state(
                            &workspace_root,
                            &self.effective_project_config(),
                            &session_id,
                        ) {
                            if let Some(worktree) = state.meta.worktree_path {
                                ui.add_space(8.0);
                                ui.label(egui::RichText::new("Worktree").size(11.0).strong());
                                ui.colored_label(
                                    palette::TEXT_DIM,
                                    widgets::truncate_path(&worktree.display().to_string(), 42),
                                );
                            }
                            if !state.meta.child_session_ids.is_empty() {
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new("Persisted Lineage").size(11.0).strong(),
                                );
                                for child in state.meta.child_session_ids.clone() {
                                    ui.horizontal(|ui| {
                                        ui.colored_label(palette::TEXT_DIM, &child);
                                        if ui.small_button("Open").clicked() {
                                            self.open_child_session(&child);
                                        }
                                    });
                                }
                            }
                        }
                    }

                    if let Some(ws) = self
                        .active_session
                        .as_ref()
                        .map(|s| s.info.workspace_root.clone())
                        .or_else(|| self.selected_workspace())
                    {
                        ui.add_space(12.0);
                        ui.label(egui::RichText::new("CLI").size(11.0).strong());
                        let cwd = ws.display().to_string();
                        let attach = format!(
                            "# cwd: {cwd}\nnca attach {}",
                            session_id
                        );
                        let status = format!(
                            "# cwd: {cwd}\nnca status {} --json",
                            session_id
                        );
                        let logs = format!("# cwd: {cwd}\nnca logs {}", session_id);
                        let cancel = format!(
                            "# cwd: {cwd}\nnca cancel {} --json",
                            session_id
                        );
                        let resume = format!(
                            "# cwd: {cwd}\nnca resume {}",
                            session_id
                        );
                        ui.horizontal_wrapped(|ui| {
                            if ui.small_button("Copy attach").clicked() {
                                ui.ctx().copy_text(attach.clone());
                                self.set_status("Copied nca attach …", false);
                            }
                            if ui.small_button("Copy status").clicked() {
                                ui.ctx().copy_text(status.clone());
                                self.set_status("Copied nca status …", false);
                            }
                            if ui.small_button("Copy logs").clicked() {
                                ui.ctx().copy_text(logs.clone());
                                self.set_status("Copied nca logs …", false);
                            }
                            if ui.small_button("Copy cancel").clicked() {
                                ui.ctx().copy_text(cancel.clone());
                                self.set_status("Copied nca cancel …", false);
                            }
                            if ui.small_button("Copy resume").clicked() {
                                ui.ctx().copy_text(resume.clone());
                                self.set_status("Copied nca resume …", false);
                            }
                        });
                    }
                } else {
                    ui.colored_label(palette::TEXT_DIM, "Open a session to inspect its lineage.");
                }
            });
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(palette::BG)
                    .inner_margin(egui::Margin::same(0.0)),
            )
            .show(ctx, |ui| {
                let Some(session) = self.active_session.as_mut() else {
                    ui.centered_and_justified(|ui| {
                        ui.colored_label(
                            palette::TEXT_DIM,
                            egui::RichText::new(
                                "No active chat. Start a new session from Projects.",
                            )
                            .size(15.0),
                        );
                    });
                    return;
                };

                // Approval bar at top if needed
                if !session.pending_approvals.is_empty() {
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(30, 20, 10))
                        .inner_margin(egui::Margin::symmetric(24.0, 10.0))
                        .stroke(egui::Stroke::new(1.0, palette::WARNING))
                        .show(ui, |ui| {
                            let approvals = session.pending_approvals.clone();
                            for a in approvals {
                                ui.horizontal(|ui| {
                                    ui.colored_label(
                                        palette::WARNING,
                                        egui::RichText::new(format!(
                                            "Approval needed: {} — {}",
                                            a.tool, a.description
                                        ))
                                        .size(12.0),
                                    );
                                    let approve_btn = egui::Button::new(
                                        egui::RichText::new("Approve")
                                            .size(11.0)
                                            .color(palette::WHITE),
                                    )
                                    .fill(palette::SUCCESS)
                                    .rounding(4.0);
                                    if ui.add(approve_btn).clicked() {
                                        session.controller.send_command(
                                            &AgentCommand::ApproveToolCall {
                                                call_id: a.call_id.clone(),
                                            },
                                        );
                                    }
                                    let deny_btn = egui::Button::new(
                                        egui::RichText::new("Deny")
                                            .size(11.0)
                                            .color(palette::WHITE),
                                    )
                                    .fill(palette::ERROR)
                                    .rounding(4.0);
                                    if ui.add(deny_btn).clicked() {
                                        session.controller.send_command(
                                            &AgentCommand::DenyToolCall {
                                                call_id: a.call_id.clone(),
                                            },
                                        );
                                    }
                                });
                            }
                        });
                }

                // Chat transcript
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        let max_w = 780.0_f32.min((ui.available_width() - 48.0).max(200.0));
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), ui.available_height()),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                ui.set_max_width(max_w);
                                ui.add_space(24.0);

                                for entry in &session.transcript {
                                    widgets::render_chat_entry(ui, entry);
                                }

                                if !session.streaming_assistant.is_empty() {
                                    widgets::render_chat_entry(
                                        ui,
                                        &ChatEntry {
                                            role: ChatRole::Assistant,
                                            title: "Orchestrator".into(),
                                            content: session.streaming_assistant.clone(),
                                        },
                                    );
                                }

                                if session.run_in_progress && session.streaming_assistant.is_empty()
                                {
                                    ui.horizontal(|ui| {
                                        ui.colored_label(
                                            palette::ACCENT,
                                            egui::RichText::new("● Agent is working...").size(12.0),
                                        );
                                    });
                                    ui.add_space(8.0);
                                }

                                if let Some(reason) = &session.ended {
                                    ui.add_space(8.0);
                                    ui.colored_label(
                                        palette::TEXT_DIM,
                                        egui::RichText::new(format!("Session ended: {:?}", reason))
                                            .size(12.0),
                                    );
                                }

                                ui.add_space(16.0);
                            },
                        );
                    });

                // Bottom input bar
                egui::TopBottomPanel::bottom("chat_input")
                    .exact_height(100.0)
                    .frame(
                        egui::Frame::none()
                            .fill(palette::BG)
                            .inner_margin(egui::Margin::symmetric(0.0, 12.0))
                            .stroke(egui::Stroke::new(1.0, palette::BORDER)),
                    )
                    .show_inside(ui, |ui| {
                        let max_w = 780.0_f32.min((ui.available_width() - 48.0).max(200.0));
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), ui.available_height()),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                ui.set_max_width(max_w);
                                ui.horizontal(|ui| {
                                    ui.colored_label(
                                        palette::ACCENT,
                                        egui::RichText::new("$").monospace().size(14.0).strong(),
                                    );
                                    ui.add_space(4.0);
                                    let resp = ui.add(
                                        egui::TextEdit::singleline(&mut session.composer)
                                            .font(egui::FontId::monospace(13.0))
                                            .desired_width(
                                                (ui.available_width() - 160.0).max(100.0),
                                            )
                                            .hint_text("Type a command or message to dispatch..."),
                                    );

                                    let enter_pressed = resp.lost_focus()
                                        && ui.input(|i| i.key_pressed(egui::Key::Enter));

                                    let can_send = !session.composer.trim().is_empty();

                                    let send_btn = egui::Button::new(
                                        egui::RichText::new("Dispatch")
                                            .size(12.0)
                                            .strong()
                                            .color(palette::WHITE),
                                    )
                                    .fill(palette::ACCENT)
                                    .rounding(8.0)
                                    .min_size(egui::vec2(90.0, 32.0));

                                    if ui.add_enabled(can_send, send_btn).clicked()
                                        || (enter_pressed && can_send)
                                    {
                                        session.run_in_progress = true;
                                        session.ended = None;
                                        session.controller.send_command(
                                            &AgentCommand::SendMessage {
                                                content: session.composer.clone(),
                                            },
                                        );
                                        session.composer.clear();
                                    }

                                    let cancel_btn = egui::Button::new(
                                        egui::RichText::new("Cancel")
                                            .size(11.0)
                                            .color(palette::TEXT_DIM),
                                    )
                                    .fill(egui::Color32::TRANSPARENT)
                                    .stroke(egui::Stroke::new(1.0, palette::BORDER))
                                    .rounding(6.0);
                                    if ui.add(cancel_btn).clicked() {
                                        session.controller.send_command(&AgentCommand::Cancel);
                                    }
                                });
                            },
                        );
                    });
            });
    }

    // -----------------------------------------------------------------------
    // Settings view
    // -----------------------------------------------------------------------
    fn show_settings_view(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(palette::BG)
                    .inner_margin(egui::Margin::symmetric(32.0, 24.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    widgets::scope_tab(
                        ui,
                        &mut self.settings_scope,
                        SettingsScope::Project,
                        "Project",
                    );
                    widgets::scope_tab(
                        ui,
                        &mut self.settings_scope,
                        SettingsScope::Global,
                        "Global",
                    );
                });
                ui.add_space(16.0);

                match self.settings_scope {
                    SettingsScope::Global => {
                        ui.colored_label(
                            palette::WHITE,
                            egui::RichText::new("Global Settings").size(18.0).strong(),
                        );
                        ui.colored_label(
                            palette::TEXT_DIM,
                            egui::RichText::new("Saved to ~/.nca/config.toml").size(11.0),
                        );
                        ui.add_space(12.0);
                        widgets::show_config_form(ui, &mut self.global_settings, false);
                        ui.add_space(12.0);
                        let btn = egui::Button::new(
                            egui::RichText::new("Save Global Settings")
                                .size(12.0)
                                .strong()
                                .color(palette::WHITE),
                        )
                        .fill(palette::ACCENT)
                        .rounding(6.0);
                        if ui.add(btn).clicked() {
                            match self.global_settings.save_global() {
                                Ok(()) => self.set_status("Saved global settings.", false),
                                Err(e) => self.set_status(e.to_string(), true),
                            }
                        }
                    }
                    SettingsScope::Project => {
                        let Some(workspace_root) = self.selected_workspace() else {
                            ui.colored_label(palette::TEXT_DIM, "Pick a project folder first.");
                            return;
                        };
                        ui.colored_label(
                            palette::WHITE,
                            egui::RichText::new("Project Settings").size(18.0).strong(),
                        );
                        ui.colored_label(
                            palette::TEXT_DIM,
                            egui::RichText::new(format!(
                                "{} — .nca/config.local.toml",
                                workspace_root.display()
                            ))
                            .size(11.0),
                        );
                        ui.add_space(12.0);
                        if let Some(config) = self.project_settings.as_mut() {
                            widgets::show_config_form(ui, config, true);
                            ui.add_space(12.0);
                            let mut save_clicked = false;
                            let mut reset_clicked = false;
                            ui.horizontal(|ui| {
                                let save_btn = egui::Button::new(
                                    egui::RichText::new("Save Project Settings")
                                        .size(12.0)
                                        .strong()
                                        .color(palette::WHITE),
                                )
                                .fill(palette::ACCENT)
                                .rounding(6.0);
                                save_clicked = ui.add(save_btn).clicked();
                                ui.add_space(8.0);
                                let reset_btn = egui::Button::new(
                                    egui::RichText::new("Reset Overrides")
                                        .size(11.0)
                                        .color(palette::TEXT_DIM),
                                )
                                .fill(egui::Color32::TRANSPARENT)
                                .stroke(egui::Stroke::new(1.0, palette::BORDER))
                                .rounding(6.0);
                                reset_clicked = ui.add(reset_btn).clicked();
                            });
                            if save_clicked {
                                match config.save_workspace_file(&workspace_root) {
                                    Ok(()) => {
                                        self.set_status("Saved project settings.", false);
                                        self.reload_selected_workspace_data();
                                    }
                                    Err(e) => self.set_status(e.to_string(), true),
                                }
                            }
                            if reset_clicked {
                                match NcaConfig::clear_workspace_file(&workspace_root) {
                                    Ok(()) => {
                                        self.set_status("Removed project override file.", false);
                                        self.reload_selected_workspace_data();
                                    }
                                    Err(e) => self.set_status(e.to_string(), true),
                                }
                            }
                        }
                    }
                }
            });
    }
}

// ---------------------------------------------------------------------------
// eframe::App
// ---------------------------------------------------------------------------

impl eframe::App for DesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply dark visuals globally
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = palette::BG;
        visuals.window_fill = palette::CARD;
        visuals.extreme_bg_color = palette::INPUT_BG;
        visuals.widgets.noninteractive.bg_fill = palette::CARD;
        visuals.widgets.inactive.bg_fill = palette::INPUT_BG;
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(35, 35, 35);
        visuals.widgets.active.bg_fill = palette::ACCENT;
        visuals.selection.bg_fill = palette::ACCENT_BG;
        visuals.selection.stroke = egui::Stroke::new(1.0, palette::ACCENT);
        ctx.set_visuals(visuals);

        self.process_live_events();
        self.show_git_confirmations(ctx);
        self.show_sidebar(ctx);
        self.show_header(ctx);

        match self.view {
            View::Dashboard => self.show_dashboard_view(ctx),
            View::Projects => self.show_projects_view(ctx),
            View::Todos => self.show_todos_view(ctx),
            View::Agents => self.show_agents_view(ctx),
            View::Chat => self.show_chat_view(ctx),
            View::Git => self.show_git_worktree_view(ctx),
            View::Settings => self.show_settings_view(ctx),
        }

        ctx.request_repaint_after(Duration::from_millis(100));
    }
}
