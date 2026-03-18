use crate::prompt::NcaPrompt;
use crate::runner::SessionRuntime;
use nca_common::config::PermissionMode;
use nca_common::event::EndReason;
use nca_core::skills::SkillCatalog;
use nca_runtime::memory_store::MemoryStore;
use reedline::{Completer, Suggestion, Emacs, Vi, Reedline, Signal, FileBackedHistory};
use std::io::Write;
use std::process::Stdio;
use tokio::process::Command;

/// Built-in REPL commands for tab-completion
const REPL_COMMANDS: &[&str] = &[
    "/help", "/status", "/agent", "/plan", "/review", "/fix", "/test", "/skills", "/memory",
    "/compact", "/models", "/mcp", "/agents", "/logs", "/attach", "/config",
    "/doctor", "/model", "/permissions", "/sessions", "/exit", "/quit", "/q",
    "/clear", "/undo", "/redo", "/diff", "/cost", "/stats",
];

/// Special input prefixes
const INPUT_PREFIXES: &[&str] = &[
    "!",  // Bash mode - run shell command directly
    "@",  // File reference - fuzzy file search
    "\\", // Multiline continuation
];

/// Agent profiles inspired by OpenCode's multi-agent system.
/// Each profile modifies behavior and system prompt emphasis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentProfile {
    /// Default full-access agent for development work
    #[default]
    Build,
    /// Read-only agent for analysis and planning - denies edits
    Plan,
    /// Focused code review agent
    Review,
    /// Bug diagnosis and fix agent
    Fix,
    /// Testing and validation agent
    Test,
}

impl AgentProfile {
    /// Get the display name for this profile (shown in prompt)
    pub fn label(&self) -> &'static str {
        match self {
            AgentProfile::Build => "build",
            AgentProfile::Plan => "plan",
            AgentProfile::Review => "review",
            AgentProfile::Fix => "fix",
            AgentProfile::Test => "test",
        }
    }

    /// Get system prompt modifier for this profile
    pub fn system_modifier(&self) -> &'static str {
        match self {
            AgentProfile::Build => "",
            AgentProfile::Plan => {
                "Profile: PLAN MODE (read-only)\n- You must not modify files or run shell commands.\n\
                 - Inspect, search, read, research the web, and propose the next steps only.\n\
                 - If asked to change code, explain what would change instead of claiming it was done."
            }
            AgentProfile::Review => {
                "Profile: REVIEW MODE\n- Focus on identifying bugs, regressions, security issues, and code quality problems.\n\
                 - Check for missing tests, edge cases, and error handling.\n\
                 - Be specific about severity: critical, major, minor, or suggestion."
            }
            AgentProfile::Fix => {
                "Profile: FIX MODE\n- Diagnose the issue thoroughly before making changes.\n\
                 - Prefer minimal, verified fixes over broad rewrites.\n\
                 - Always explain the root cause and the fix."
            }
            AgentProfile::Test => {
                "Profile: TEST MODE\n- Focus on validating code correctness and edge cases.\n\
                 - Run tests, checks, or lints when tools allow.\n\
                 - Report clearly what passed, what failed, and any issues found."
            }
        }
    }

    /// Get reedline suggestion color for this profile
    pub fn style(&self) -> &'static str {
        match self {
            AgentProfile::Build => "",
            AgentProfile::Plan => "cyan",
            AgentProfile::Review => "yellow",
            AgentProfile::Fix => "red",
            AgentProfile::Test => "green",
        }
    }

    /// Cycle to the next profile (for Tab switching)
    pub fn next(self) -> Self {
        match self {
            AgentProfile::Build => AgentProfile::Plan,
            AgentProfile::Plan => AgentProfile::Review,
            AgentProfile::Review => AgentProfile::Fix,
            AgentProfile::Fix => AgentProfile::Test,
            AgentProfile::Test => AgentProfile::Build,
        }
    }

    /// All profiles in cycle order
    pub const ALL: [Self; 5] = [
        Self::Build,
        Self::Plan,
        Self::Review,
        Self::Fix,
        Self::Test,
    ];
}

impl std::fmt::Display for AgentProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Session state for REPL
pub struct Repl {
    runtime: SessionRuntime,
    prompt: NcaPrompt,
    run_mode: bool,
    history_path: std::path::PathBuf,
    agent_profile: AgentProfile,
    current_agent_label: String,
}

impl Repl {
    pub fn new(runtime: SessionRuntime, safe_mode: bool, run_mode: bool) -> Self {
        let history_path = runtime.workspace_root().join(".nca/.history");
        let agent_profile = AgentProfile::default();
        let current_agent_label = format!("@{}", agent_profile.label());
        Self {
            runtime,
            prompt: NcaPrompt::new(safe_mode, run_mode),
            run_mode,
            history_path,
            agent_profile,
            current_agent_label,
        }
    }

    /// Run the interactive REPL until the user exits.
    pub async fn run(&mut self) -> anyhow::Result<()> {
        let mut editor = self.build_editor()?;

        if self.run_mode {
            self.print_banner();
        }

        loop {
            // Update prompt with current agent profile
            self.prompt.set_agent(&self.current_agent_label);
            let sig = editor.read_line(&self.prompt);
            match sig {
                Ok(Signal::Success(input)) => {
                    if input.is_empty() {
                        continue;
                    }

                    // Tab switches agent profile (OpenCode-style)
                    if input == "\t" {
                        self.switch_agent();
                        continue;
                    }

                    // Bash mode: ! prefix runs shell command directly
                    if input.starts_with('!') {
                        let cmd = input.trim_start_matches('!');
                        self.run_bash_command(cmd).await;
                        continue;
                    }

                    // File reference: @ prefix for fuzzy file search
                    if input.starts_with('@') {
                        let query = input.trim_start_matches('@');
                        self.handle_file_reference(query).await;
                        continue;
                    }

                    // Slash commands
                    if input.starts_with('/') {
                        if !self.handle_command(&input).await? {
                            break;
                        }
                        continue;
                    }

                    // Regular input to agent
                    match self.runtime.run_turn(&input).await {
                        Ok(output) => {
                            println!("{output}");
                        }
                        Err(err) => {
                            eprintln!("error: {err}");
                        }
                    }
                }
                Ok(Signal::CtrlD) => {
                    // Ctrl+D - exit
                    eprintln!("\n[exit]");
                    break;
                }
                Ok(Signal::CtrlC) => {
                    // Ctrl+C - cancel current or exit
                    eprintln!("\n[cancel] Press Ctrl+D to exit, or wait for current operation to complete");
                }
                Err(err) => {
                    eprintln!("read error: {err}");
                    break;
                }
            }
        }

        self.runtime.finish(EndReason::UserExit).await;
        Ok(())
    }

    fn print_banner(&self) {
        eprintln!(
            r#"
╔══════════════════════════════════════════════════════════════╗
║  nca - Native CLI AI                                          ║
║  Interactive terminal mode                                     ║
╠══════════════════════════════════════════════════════════════╣
║  Shortcuts:                                                   ║
║    ! <cmd>   Run shell command (bash mode)                    ║
║    @ <file>  Reference a file                                 ║
║    / <cmd>   Slash commands                                  ║
║    Tab       Switch agent profile (@build/@plan/@review...)   ║
║    Ctrl+D    Exit                                            ║
║    Ctrl+C    Cancel current request                           ║
║    Ctrl+L    Clear screen                                     ║
║    Ctrl+R    Search command history                           ║
╚══════════════════════════════════════════════════════════════╝
"#
        );
    }

    /// Switch to the next agent profile (called on Tab press)
    fn switch_agent(&mut self) {
        let next = self.agent_profile.next();
        self.agent_profile = next;
        self.current_agent_label = format!("@{}", next.label());
        self.prompt.set_agent(&self.current_agent_label);

        // Update runtime permission mode based on profile
        if next == AgentProfile::Plan {
            self.runtime.set_permission_mode(PermissionMode::Plan);
        }

        eprintln!("\n[agent] Switched to @{} mode", next.label());
        if next == AgentProfile::Plan {
            eprintln!("[agent] Plan mode: file edits and shell commands are disabled");
        }
    }

    /// Run a shell command directly (bash mode) - Claude Code style
    /// Output is returned to the conversation context
    async fn run_bash_command(&self, cmd: &str) {
        let cmd = cmd.trim();
        if cmd.is_empty() {
            eprintln!("! usage: !<command> [args]");
            return;
        }

        eprintln!("[bash] {cmd}");

        let output = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);

                if !stdout.is_empty() {
                    println!("{stdout}");
                }
                if !stderr.is_empty() {
                    eprintln!("[stderr] {stderr}");
                }
                if out.status.success() {
                    eprintln!("[bash] completed (exit 0)");
                } else {
                    eprintln!("[bash] failed (exit {})", out.status.code().unwrap_or(-1));
                }
            }
            Err(e) => {
                eprintln!("[bash] failed to execute: {e}");
            }
        }
    }

    /// Handle file reference (@ prefix) - OpenCode style
    /// Performs fuzzy file search and shows matching files
    async fn handle_file_reference(&self, query: &str) {
        let query = query.trim();
        let workspace = self.runtime.workspace_root();

        eprintln!("[file] Searching for: {query}");

        // Build find command for fuzzy search
        let find_cmd = if query.is_empty() {
            format!(
                "find . -type f -name '*.rs' -o -name '*.ts' -o -name '*.js' -o -name '*.py' -o -name '*.json' 2>/dev/null | head -20"
            )
        } else {
            // Escape special characters for grep
            let escaped = query.replace(|c: char| !c.is_alphanumeric() && c != '.' && c != '-' && c != '_', "\\");
            format!(
                "find . -type f \\( -name '*{escaped}*' -o -path '*{escaped}*' \\) 2>/dev/null | head -20"
            )
        };

        let output = Command::new("sh")
            .arg("-c")
            .arg(&find_cmd)
            .current_dir(workspace)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        match output {
            Ok(out) => {
                let files = String::from_utf8_lossy(&out.stdout);
                if files.is_empty() {
                    eprintln!("[file] No files found matching: {query}");
                } else {
                    eprintln!("[file] Matches:");
                    for (i, line) in files.lines().enumerate() {
                        if !line.is_empty() {
                            println!("  {}: {}", i + 1, line);
                        }
                    }
                    eprintln!("\n[file] Reference files in your prompt using @<number> or @<path>");
                }
            }
            Err(e) => {
                eprintln!("[file] Search failed: {e}");
            }
        }
    }

    /// Open external editor for long prompts (Ctrl+G style)
    async fn open_external_editor(&self) -> Option<String> {
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());

        // Create a temp file
        let temp_path = std::env::temp_dir().join("nca-prompt-XXXXXX");
        let temp_path_str = temp_path.to_string_lossy().to_string();

        // Use mktemp-like approach
        let temp_file = format!("{}.txt", std::process::id());
        let temp_path = std::env::temp_dir().join(&temp_file);

        // Write current buffer if any
        std::fs::write(&temp_path, "").ok()?;

        // Spawn editor
        let output = Command::new("sh")
            .arg("-c")
            .arg(format!("{} '{}'", editor, temp_path.display()))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        match output {
            Ok(_) => {
                let content = std::fs::read_to_string(&temp_path).ok()?;
                let _ = std::fs::remove_file(&temp_path);
                let content = content.trim().to_string();
                if content.is_empty() {
                    None
                } else {
                    Some(content)
                }
            }
            Err(e) => {
                eprintln!("[editor] Failed to open: {e}");
                None
            }
        }
    }

    fn build_editor(&self) -> anyhow::Result<Reedline> {
        let mut builder = Reedline::create()
            .with_quick_completions(true)
            .with_partial_completions(true)
            .with_ansi_colors(true);

        // Try to load history from disk
        if let Some(parent) = self.history_path.parent() {
            std::fs::create_dir_all(parent).ok();
            if let Ok(history) = FileBackedHistory::with_file(100, self.history_path.clone()) {
                builder = builder.with_history(Box::new(history));
            }
        }

        // Support vim mode if enabled via env
        if std::env::var("NCA_EDITOR_MODE")
            .map(|v| v.eq_ignore_ascii_case("vi") || v.eq_ignore_ascii_case("vim"))
            .unwrap_or(false)
        {
            builder = builder.with_edit_mode(Box::new(Vi::default()));
        } else {
            builder = builder.with_edit_mode(Box::new(Emacs::default()));
        }

        Ok(builder)
    }

    async fn handle_command(&mut self, input: &str) -> anyhow::Result<bool> {
        let mut parts = input.split_whitespace();
        let command = parts.next().unwrap_or_default();
        let rest = input
            .strip_prefix(command)
            .map(str::trim)
            .unwrap_or_default();

        match command {
            "/q" | "/quit" | "/exit" => return Ok(false),
            "/help" => {
                print!(
                    "nca Interactive Mode - Claude Code inspired shortcuts:\n\n\
                     INPUT MODES:\n\
                       ! <cmd>     Run shell command directly (output feeds back to context)\n\
                       @ <query>   Search and reference files\n\
                       / <cmd>     Slash commands\n\
                       \\          Multiline input (end line with \\ to continue)\n\n\
                     SLASH COMMANDS:\n\
                       /help                       Show this help\n\
                       /status                     Show current session status\n\
                       /agent [profile]           Show or switch agent profile\n\
                       /plan <task>               Run a planning-oriented turn\n\
                       /review <task>             Review code or changes\n\
                       /fix <task>                Run a bug-fix oriented turn\n\
                       /test <task>               Ask the agent to validate/test\n\
                       /clear                     Clear the screen\n\
                       /compact                   Save a compact session summary\n\
                       /undo                      Undo last agent response\n\
                       /redo                      Redo undone response\n\
                       /diff                      Show recent file changes\n\
                       /cost                      Show token usage and cost\n\
                       /stats                     Show session statistics\n\
                       /skills                    List discovered skills\n\
                       /memory [text]             Show or store workspace memory\n\
                       /models                    Show available models\n\
                       /mcp                       List configured MCP servers\n\
                       /agents                    Show child sessions\n\
                       /logs                      Print the current event log\n\
                       /attach                    Show current attach target\n\
                       /config                    Show effective runtime config\n\
                       /doctor                    Run MiniMax config checks\n\
                       /sessions                  List local session IDs\n\
                       /exit                      Exit repl\n\n\
                     KEYBOARD SHORTCUTS:\n\
                       Tab                         Switch agent profile (@build -> @plan -> @review)\n\
                       Ctrl+D                     Exit repl\n\
                       Ctrl+C                     Cancel current request\n\
                       Ctrl+L                     Clear screen\n\
                       Ctrl+R                     Search command history\n"
                );
            }
            "/status" => {
                let snapshot = self.runtime.snapshot();
                println!(
                    "session={} model={} agent={} permission_mode={:?} children={} memory={}",
                    snapshot.id,
                    self.runtime.model(),
                    self.agent_profile.label(),
                    self.runtime.permission_mode(),
                    snapshot.child_session_ids.len(),
                    self.runtime.memory_store_path().display()
                );
                if let Some(summary) = snapshot.session_summary {
                    println!("summary: {}", summary.replace('\n', " "));
                }
            }
            "/agent" => {
                if let Some(target) = parts.next() {
                    let target_clean = target.trim_start_matches('@').to_lowercase();
                    let matched = AgentProfile::ALL.iter().find(|p| {
                        p.label() == target_clean
                    });
                    if let Some(profile) = matched {
                        self.agent_profile = *profile;
                        self.current_agent_label = format!("@{}", profile.label());
                        self.prompt.set_agent(&self.current_agent_label);
                        if *profile == AgentProfile::Plan {
                            self.runtime.set_permission_mode(PermissionMode::Plan);
                        }
                        println!("Switched to @{} mode", profile.label());
                    } else {
                        println!("Unknown agent profile: {}", target);
                        println!("Available: {}", AgentProfile::ALL.iter().map(|p| p.label()).collect::<Vec<_>>().join(", "));
                    }
                } else {
                    println!("Current agent: @{}", self.agent_profile.label());
                    println!("Available profiles:");
                    for profile in AgentProfile::ALL {
                        let marker = if profile == self.agent_profile { " *" } else { "" };
                        println!("  @{}{}", profile.label(), marker);
                    }
                }
            }
            "/plan" => self
                .run_preset(
                    "Create a short implementation plan before coding. Focus on steps, risks, and validation.\n\nTask:\n",
                    rest,
                )
                .await?,
            "/review" => self
                .run_preset(
                    "Review the requested code or changes. Prioritize bugs, regressions, risks, and missing tests.\n\nReview target:\n",
                    rest,
                )
                .await?,
            "/fix" => self
                .run_preset(
                    "Diagnose and fix the issue below. Prefer a minimal verified change.\n\nIssue:\n",
                    rest,
                )
                .await?,
            "/test" => self
                .run_preset(
                    "Validate the requested area. Run tests or checks if tools allow, and report what passed or failed.\n\nTarget:\n",
                    rest,
                )
                .await?,
            "/model" => {
                if let Some(model) = parts.next() {
                    let resolved = self.runtime.config().model.resolve_alias(model);
                    self.runtime.set_model(resolved.clone());
                    println!("model set to {resolved}");
                } else {
                    println!("model: {}", self.runtime.model());
                }
            }
            "/clear" => {
                print!("\x1B[2J\x1B[H");
                std::io::stdout().flush().ok();
                println!("[screen cleared]");
            }
            "/undo" => {
                // Undo last agent response - would need runtime support
                eprintln!("[undo] Not yet implemented - use /compact to save session state");
            }
            "/redo" => {
                // Redo undone response - would need runtime support
                eprintln!("[redo] Not yet implemented");
            }
            "/diff" => {
                // Show recent file changes via git
                let output = Command::new("sh")
                    .arg("-c")
                    .arg("git diff --stat HEAD~5..HEAD 2>/dev/null || git diff --stat 2>/dev/null || echo 'No git changes'")
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .await;
                match output {
                    Ok(out) => {
                        let diff = String::from_utf8_lossy(&out.stdout);
                        if diff.is_empty() {
                            println!("[diff] No recent changes");
                        } else {
                            print!("{diff}");
                        }
                    }
                    Err(e) => eprintln!("[diff] Failed: {e}"),
                }
            }
            "/cost" => {
                let snapshot = self.runtime.snapshot();
                eprintln!("[cost] Session: {}", snapshot.id);
                eprintln!("[cost] Use 'nca logs --follow' to see real-time token usage");
            }
            "/stats" => {
                let snapshot = self.runtime.snapshot();
                println!("session_id: {}", snapshot.id);
                println!("model: {}", self.runtime.model());
                println!("agent: @{}", self.agent_profile.label());
                println!("permission_mode: {:?}", self.runtime.permission_mode());
                println!("child_sessions: {}", snapshot.child_session_ids.len());
                println!("memory_path: {}", self.runtime.memory_store_path().display());
            }
            "/permissions" => {
                if let Some(mode) = parts.next() {
                    if let Some(parsed_mode) = parse_permission_mode(mode) {
                        self.runtime.set_permission_mode(parsed_mode);
                        println!("permission mode set to {parsed_mode:?}");
                    } else {
                        println!(
                            "invalid mode; expected one of: default, plan, accept-edits, dont-ask, bypass-permissions"
                        );
                    }
                } else {
                    println!("permission_mode: {:?}", self.runtime.permission_mode());
                }
            }
            "/skills" => {
                let skills = SkillCatalog::discover(
                    self.runtime.workspace_root(),
                    &self.runtime.config().harness.skill_directories,
                )
                .map_err(anyhow::Error::msg)?;
                if skills.is_empty() {
                    println!("no skills discovered");
                } else {
                    for skill in skills {
                        println!("{}", skill.summary_line());
                    }
                }
            }
            "/memory" => {
                if rest.is_empty() {
                    let store = MemoryStore::new(self.runtime.memory_store_path());
                    let state = store.load().await.map_err(anyhow::Error::msg)?;
                    if state.notes.is_empty() {
                        println!("no memory notes stored");
                    } else {
                        for note in state.notes.iter().rev().take(5) {
                            println!(
                                "{} {} {}",
                                note.id,
                                note.kind,
                                note.content.replace('\n', " ")
                            );
                        }
                    }
                } else {
                    self.runtime
                        .append_memory_note("note", Some(rest.to_string()))
                        .await
                        .map_err(anyhow::Error::msg)?;
                    println!("memory note saved");
                }
            }
            "/compact" => {
                let summary = self.runtime.compact_summary();
                self.runtime.set_session_summary(Some(summary.clone()));
                self.runtime
                    .append_memory_note("session-summary", Some(summary.clone()))
                    .await
                    .map_err(anyhow::Error::msg)?;
                self.runtime.save().await.map_err(anyhow::Error::msg)?;
                println!("saved session summary:\n{}", summary);
            }
            "/models" => {
                let provider = self.runtime.config().provider.default;
                println!(
                    "default_provider={} default_model={} thinking={} budget={}",
                    provider.display_name(),
                    self.runtime.config().model.default_model,
                    self.runtime.config().model.enable_thinking,
                    self.runtime.config().model.thinking_budget
                );
                for provider in nca_common::config::ProviderKind::ALL {
                    println!(
                        "  {} -> {} ({})",
                        provider.display_name(),
                        self.runtime.config().provider.model_for(provider),
                        self.runtime.config().provider.base_url_for(provider)
                    );
                }
                for (alias, target) in &self.runtime.config().model.aliases {
                    println!("  {alias} -> {target}");
                }
            }
            "/mcp" => {
                if self.runtime.config().mcp.servers.is_empty() {
                    println!("no MCP servers configured");
                } else {
                    for server in self.runtime.config().mcp.servers.iter().filter(|server| server.enabled) {
                        println!(
                            "{} command={} {}",
                            server.name,
                            server.command,
                            server.args.join(" ")
                        );
                    }
                }
            }
            "/agents" => {
                let snapshot = self.runtime.snapshot();
                if snapshot.child_session_ids.is_empty() {
                    println!("no child sessions yet");
                } else {
                    for child in snapshot.child_session_ids {
                        println!("{child}");
                    }
                }
            }
            "/logs" => {
                match tokio::fs::read_to_string(self.runtime.event_log_path()).await {
                    Ok(data) => print!("{data}"),
                    Err(err) => {
                        eprintln!("failed to read log: {err}")
                    }
                }
            }
            "/attach" => {
                let snapshot = self.runtime.snapshot();
                println!(
                    "session={} socket={}",
                    snapshot.id,
                    snapshot
                        .socket_path
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "<none>".into())
                );
            }
            "/config" => {
                let config = self.runtime.config();
                println!(
                    "provider={:?} model={} permission_mode={:?} memory={}",
                    config.provider.default,
                    self.runtime.model(),
                    self.runtime.permission_mode(),
                    self.runtime.memory_store_path().display()
                );
            }
            "/doctor" => {
                for provider in nca_common::config::ProviderKind::ALL {
                    let configured = self
                        .runtime
                        .config()
                        .provider
                        .api_key_present_for(provider);
                    println!(
                        "{}{} API key {} ({})",
                        provider.display_name(),
                        if provider == self.runtime.config().provider.default {
                            " [selected]"
                        } else {
                            ""
                        },
                        if configured { "configured" } else { "missing" },
                        self.runtime.config().provider.api_key_env_for(provider)
                    );
                }
            }
            "/sessions" => match self.runtime.list_session_ids().await {
                Ok(mut ids) => {
                    ids.sort();
                    if ids.is_empty() {
                        println!("no saved sessions");
                    } else {
                        for id in ids {
                            println!("{id}");
                        }
                    }
                }
                Err(error) => {
                    eprintln!("failed to list sessions: {error}");
                }
            },
            _ => {
                if command.starts_with('/') {
                    if self.try_run_skill(command.trim_start_matches('/'), rest).await? {
                        return Ok(true);
                    }
                }
                eprintln!("unknown command: {command}");
            }
        }

        Ok(true)
    }

    async fn run_preset(&mut self, prefix: &str, task: &str) -> anyhow::Result<()> {
        if task.trim().is_empty() {
            println!("usage: /<command> <task description>");
            return Ok(());
        }
        let prompt = format!("{prefix}{}", task.trim());
        match self.runtime.run_turn(&prompt).await {
            Ok(output) => {
                println!("{output}");
            }
            Err(err) => {
                eprintln!("error: {err}");
            }
        }
        Ok(())
    }

    async fn try_run_skill(&mut self, skill_name: &str, task: &str) -> anyhow::Result<bool> {
        let skills = SkillCatalog::discover(
            self.runtime.workspace_root(),
            &self.runtime.config().harness.skill_directories,
        )
        .map_err(anyhow::Error::msg)?;
        let Some(skill) = skills.into_iter().find(|skill| skill.command == skill_name) else {
            return Ok(false);
        };

        if let Some(model) = &skill.model {
            self.runtime
                .set_model(self.runtime.config().model.resolve_alias(model));
        }
        if let Some(mode) = skill.permission_mode {
            self.runtime.set_permission_mode(mode);
        }

        let prompt = skill.prompt_for_task(task);
        match self.runtime.run_turn(&prompt).await {
            Ok(output) => {
                println!("{output}");
            }
            Err(err) => {
                eprintln!("error: {err}");
            }
        }
        Ok(true)
    }
}

/// Tab completion for REPL commands and skills
impl Completer for Repl {
    fn complete(&mut self, line: &str, _pos: usize) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();

        // Complete REPL commands starting with /
        if line.starts_with('/') {
            for cmd in REPL_COMMANDS {
                if cmd.starts_with(line) {
                    suggestions.push(Suggestion {
                        value: cmd.to_string(),
                        description: Some("REPL command".to_string()),
                        extra: None,
                        span: reedline::Span { start: 0, end: 0 },
                        append_whitespace: true,
                        style: None,
                    });
                }
            }
        }

        // Complete bash mode commands (starting with !)
        if line.starts_with('!') {
            // Common shell commands
            let bash_commands = ["git", "ls", "cat", "find", "grep", "npm", "cargo", "make", "docker", "curl"];
            let prefix = line.trim_start_matches('!');
            for cmd in bash_commands {
                let full = format!("!{}", cmd);
                if full.starts_with(line) {
                    suggestions.push(Suggestion {
                        value: full,
                        description: Some("Shell command".to_string()),
                        extra: None,
                        span: reedline::Span { start: 0, end: 0 },
                        append_whitespace: true,
                        style: None,
                    });
                }
            }
        }

        // Complete file references (starting with @)
        if line.starts_with('@') {
            let prefix = line.trim_start_matches('@');
            // Suggest some common file patterns
            let patterns = ["src/", "lib/", "tests/", "docs/", "Cargo.toml", "package.json", "README.md"];
            for pat in patterns {
                if pat.starts_with(prefix) {
                    suggestions.push(Suggestion {
                        value: format!("@{}", pat),
                        description: Some("File reference".to_string()),
                        extra: None,
                        span: reedline::Span { start: 0, end: 0 },
                        append_whitespace: true,
                        style: None,
                    });
                }
            }
        }

        // Load skills for completion
        if let Ok(skills) = SkillCatalog::discover(
            self.runtime.workspace_root(),
            &self.runtime.config().harness.skill_directories,
        ) {
            for skill in skills {
                let skill_cmd = format!("/{}", skill.command);
                if skill_cmd.starts_with(line) {
                    suggestions.push(Suggestion {
                        value: skill_cmd,
                        description: skill.description,
                        extra: None,
                        span: reedline::Span { start: 0, end: 0 },
                        append_whitespace: true,
                        style: None,
                    });
                }
            }
        }

        suggestions
    }
}

fn parse_permission_mode(raw: &str) -> Option<PermissionMode> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "default" => Some(PermissionMode::Default),
        "plan" => Some(PermissionMode::Plan),
        "accept-edits" | "accept_edits" | "acceptedits" => Some(PermissionMode::AcceptEdits),
        "dont-ask" | "dont_ask" | "dontask" => Some(PermissionMode::DontAsk),
        "bypass-permissions" | "bypass_permissions" | "bypasspermissions" => {
            Some(PermissionMode::BypassPermissions)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_permission_aliases() {
        assert_eq!(
            parse_permission_mode("accept-edits"),
            Some(PermissionMode::AcceptEdits)
        );
        assert_eq!(
            parse_permission_mode("dontask"),
            Some(PermissionMode::DontAsk)
        );
        assert_eq!(
            parse_permission_mode("bypass_permissions"),
            Some(PermissionMode::BypassPermissions)
        );
        assert_eq!(parse_permission_mode("invalid"), None);
    }
}