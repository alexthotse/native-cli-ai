use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

/// Top-level configuration, merged from global, workspace, env, and CLI sources.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NcaConfig {
    pub provider: ProviderConfig,
    pub model: ModelConfig,
    pub permissions: PermissionConfig,
    pub session: SessionConfig,
    pub harness: HarnessConfig,
    pub mcp: McpConfig,
    pub memory: MemoryConfig,
    pub hooks: HookConfig,
    pub web: WebConfig,
    /// CLI/TUI preferences (e.g. external editor).
    #[serde(default)]
    pub ui: UiConfig,
}

impl NcaConfig {
    /// Load config from defaults, global file, workspace file, and environment.
    pub fn load() -> Result<Self, ConfigError> {
        let workspace_root = env::current_dir().map_err(|source| ConfigError::Io {
            action: "read current directory",
            path: PathBuf::from("."),
            source,
        })?;
        Self::load_for_workspace(&workspace_root)
    }

    /// Load config for an explicit workspace root.
    pub fn load_for_workspace(workspace_root: &Path) -> Result<Self, ConfigError> {
        let mut config = Self::default();

        if let Some(path) = global_config_path()
            && path.exists()
        {
            let partial = load_partial(&path)?;
            config.merge(partial);
        }

        let local_path = workspace_config_path(workspace_root);
        if local_path.exists() {
            let partial = load_partial(&local_path)?;
            config.merge(partial);
        }

        config.apply_env();
        Ok(config)
    }

    /// Load only the persisted global config file layered over defaults.
    pub fn load_global_file() -> Result<Self, ConfigError> {
        let mut config = Self::default();
        if let Some(path) = global_config_path()
            && path.exists()
        {
            let partial = load_partial(&path)?;
            config.merge(partial);
        }
        Ok(config)
    }

    /// Load only the persisted workspace-local config layered over defaults.
    pub fn load_workspace_file(workspace_root: &Path) -> Result<Self, ConfigError> {
        let mut config = Self::default();
        let local_path = workspace_config_path(workspace_root);
        if local_path.exists() {
            let partial = load_partial(&local_path)?;
            config.merge(partial);
        }
        Ok(config)
    }

    /// Save the full config as the user's global defaults.
    pub fn save_global(&self) -> Result<(), ConfigError> {
        let path = global_config_path().ok_or(ConfigError::NoHomeDir)?;
        save_config_to_path(self, &path)
    }

    /// Save the full config as the workspace-local override file.
    pub fn save_workspace_file(&self, workspace_root: &Path) -> Result<(), ConfigError> {
        let path = workspace_config_path(workspace_root);
        save_config_to_path(self, &path)
    }

    /// Remove the workspace-local config file, if present.
    pub fn clear_workspace_file(workspace_root: &Path) -> Result<(), ConfigError> {
        let path = workspace_config_path(workspace_root);
        if !path.exists() {
            return Ok(());
        }
        std::fs::remove_file(&path).map_err(|source| ConfigError::Io {
            action: "remove config file",
            path,
            source,
        })
    }

    fn merge(&mut self, partial: PartialNcaConfig) {
        let provider_changed = partial.provider.is_some();
        let explicit_model_override = partial
            .model
            .as_ref()
            .and_then(|model| model.default_model.as_ref())
            .is_some();
        if let Some(provider) = partial.provider {
            self.provider.merge(provider);
        }

        if let Some(model) = partial.model {
            self.model.merge(model);
        }

        if let Some(permissions) = partial.permissions {
            self.permissions.merge(permissions);
        }

        if let Some(session) = partial.session {
            self.session.merge(session);
        }
        if let Some(harness) = partial.harness {
            self.harness.merge(harness);
        }
        if let Some(mcp) = partial.mcp {
            self.mcp.merge(mcp);
        }
        if let Some(memory) = partial.memory {
            self.memory.merge(memory);
        }
        if let Some(hooks) = partial.hooks {
            self.hooks.merge(hooks);
        }
        if let Some(web) = partial.web {
            self.web.merge(web);
        }
        if let Some(ui) = partial.ui {
            self.ui.merge(ui);
        }

        if explicit_model_override {
            self.provider
                .set_model_for_default(self.model.default_model.clone());
        }

        if provider_changed || explicit_model_override {
            self.sync_default_model_from_provider();
        }
    }

    fn apply_env(&mut self) {
        if let Ok(provider) = env::var("NCA_DEFAULT_PROVIDER") {
            self.provider.default = ProviderKind::from_env(&provider);
            self.sync_default_model_from_provider();
        }

        if let Ok(model) = env::var("NCA_MODEL") {
            self.apply_model_override(&model);
        }

        if let Ok(api_key) = env::var("MINIMAX_API_KEY") {
            self.provider.minimax.api_key = Some(api_key);
        }

        if let Ok(base_url) = env::var("MINIMAX_BASE_URL") {
            self.provider.minimax.base_url = base_url;
        }

        if let Ok(model) = env::var("MINIMAX_MODEL") {
            self.provider.minimax.model = model;
        }

        if let Ok(api_key) = env::var("OPENAI_API_KEY") {
            self.provider.openai.api_key = Some(api_key);
        }

        if let Ok(base_url) = env::var("OPENAI_BASE_URL") {
            self.provider.openai.base_url = base_url;
        }

        if let Ok(model) = env::var("OPENAI_MODEL") {
            self.provider.openai.model = model;
        }

        if let Ok(api_key) = env::var("ANTHROPIC_API_KEY") {
            self.provider.anthropic.api_key = Some(api_key);
        }

        if let Ok(base_url) = env::var("ANTHROPIC_BASE_URL") {
            self.provider.anthropic.base_url = base_url;
        }

        if let Ok(model) = env::var("ANTHROPIC_MODEL") {
            self.provider.anthropic.model = model;
        }

        if let Ok(api_key) = env::var("OPENROUTER_API_KEY") {
            self.provider.openrouter.api_key = Some(api_key);
        }

        if let Ok(base_url) = env::var("OPENROUTER_BASE_URL") {
            self.provider.openrouter.base_url = base_url;
        }

        if let Ok(model) = env::var("OPENROUTER_MODEL") {
            self.provider.openrouter.model = model;
        }

        if let Ok(site_url) = env::var("OPENROUTER_SITE_URL") {
            self.provider.openrouter.site_url = Some(site_url);
        }

        if let Ok(app_name) = env::var("OPENROUTER_APP_NAME") {
            self.provider.openrouter.app_name = Some(app_name);
        }

        // NVIDIA NIM environment variables
        if let Ok(api_key) = env::var("NVIDIA_API_KEY") {
            self.provider.nvidianim.api_key = Some(api_key);
        }
        if let Ok(base_url) = env::var("NVIDIA_BASE_URL") {
            self.provider.nvidianim.base_url = base_url;
        }
        if let Ok(model) = env::var("NVIDIA_MODEL") {
            self.provider.nvidianim.model = model;
        }

        // OpenCode environment variables
        if let Ok(api_key) = env::var("OPENCODE_API_KEY") {
            self.provider.opcode.api_key = Some(api_key);
        }
        if let Ok(base_url) = env::var("OPENCODE_BASE_URL") {
            self.provider.opcode.base_url = base_url;
        }
        if let Ok(model) = env::var("OPENCODE_MODEL") {
            self.provider.opcode.model = model;
        }

        // GLM environment variables
        if let Ok(api_key) = env::var("GLM_API_KEY") {
            self.provider.glm.api_key = Some(api_key);
        }
        if let Ok(base_url) = env::var("GLM_BASE_URL") {
            self.provider.glm.base_url = base_url;
        }
        if let Ok(model) = env::var("GLM_MODEL") {
            self.provider.glm.model = model;
        }

        // Kimi environment variables
        if let Ok(api_key) = env::var("KIMI_API_KEY") {
            self.provider.kimi.api_key = Some(api_key);
        }
        if let Ok(base_url) = env::var("KIMI_BASE_URL") {
            self.provider.kimi.base_url = base_url;
        }
        if let Ok(model) = env::var("KIMI_MODEL") {
            self.provider.kimi.model = model;
        }

        // KiloCode environment variables
        if let Ok(api_key) = env::var("KILOCODE_API_KEY") {
            self.provider.kilocode.api_key = Some(api_key);
        }
        if let Ok(base_url) = env::var("KILOCODE_BASE_URL") {
            self.provider.kilocode.base_url = base_url;
        }
        if let Ok(model) = env::var("KILOCODE_MODEL") {
            self.provider.kilocode.model = model;
        }

        // Ollama environment variables
        if let Ok(api_key) = env::var("OLLAMA_API_KEY") {
            self.provider.ollama.api_key = Some(api_key);
        }
        if let Ok(base_url) = env::var("OLLAMA_BASE_URL") {
            self.provider.ollama.base_url = base_url;
        }
        if let Ok(model) = env::var("OLLAMA_MODEL") {
            self.provider.ollama.model = model;
        }

        // Groq environment variables
        if let Ok(api_key) = env::var("GROQ_API_KEY") {
            self.provider.groq.api_key = Some(api_key);
        }
        if let Ok(base_url) = env::var("GROQ_BASE_URL") {
            self.provider.groq.base_url = base_url;
        }
        if let Ok(model) = env::var("GROQ_MODEL") {
            self.provider.groq.model = model;
        }

        // Together AI environment variables
        if let Ok(api_key) = env::var("TOGETHER_API_KEY") {
            self.provider.together.api_key = Some(api_key);
        }
        if let Ok(base_url) = env::var("TOGETHER_BASE_URL") {
            self.provider.together.base_url = base_url;
        }
        if let Ok(model) = env::var("TOGETHER_MODEL") {
            self.provider.together.model = model;
        }

        // Fireworks AI environment variables
        if let Ok(api_key) = env::var("FIREWORKS_API_KEY") {
            self.provider.fireworks.api_key = Some(api_key);
        }
        if let Ok(base_url) = env::var("FIREWORKS_BASE_URL") {
            self.provider.fireworks.base_url = base_url;
        }
        if let Ok(model) = env::var("FIREWORKS_MODEL") {
            self.provider.fireworks.model = model;
        }

        // DeepSeek environment variables
        if let Ok(api_key) = env::var("DEEPSEEK_API_KEY") {
            self.provider.deepseek.api_key = Some(api_key);
        }
        if let Ok(base_url) = env::var("DEEPSEEK_BASE_URL") {
            self.provider.deepseek.base_url = base_url;
        }
        if let Ok(model) = env::var("DEEPSEEK_MODEL") {
            self.provider.deepseek.model = model;
        }

        // Cohere environment variables
        if let Ok(api_key) = env::var("COHERE_API_KEY") {
            self.provider.cohere.api_key = Some(api_key);
        }
        if let Ok(base_url) = env::var("COHERE_BASE_URL") {
            self.provider.cohere.base_url = base_url;
        }
        if let Ok(model) = env::var("COHERE_MODEL") {
            self.provider.cohere.model = model;
        }

        if let Ok(memory_path) = env::var("NCA_MEMORY_PATH") {
            self.memory.file_path = PathBuf::from(memory_path);
        }

        if let Ok(timeout_secs) = env::var("NCA_WEB_TIMEOUT_SECS")
            && let Ok(timeout_secs) = timeout_secs.parse()
        {
            self.web.timeout_secs = timeout_secs;
        }

        if let Ok(max_fetch_chars) = env::var("NCA_WEB_MAX_FETCH_CHARS")
            && let Ok(max_fetch_chars) = max_fetch_chars.parse()
        {
            self.web.max_fetch_chars = max_fetch_chars;
        }

        self.sync_default_model_from_provider();
    }

    pub fn apply_model_override(&mut self, raw_model: &str) {
        let resolved = self.model.resolve_alias(raw_model);
        self.provider.set_model_for_default(resolved);
        self.sync_default_model_from_provider();
    }

    /// Switch the default LLM provider and keep `default_model` aligned with that provider's model field.
    pub fn set_default_provider(&mut self, provider: ProviderKind) {
        self.provider.default = provider;
        self.sync_default_model_from_provider();
    }

    /// Set the API key stored in config for a provider (workspace save may persist it).
    pub fn set_provider_api_key(&mut self, provider: ProviderKind, key: impl Into<String>) {
        let key = key.into();
        match provider {
            ProviderKind::MiniMax => self.provider.minimax.api_key = Some(key),
            ProviderKind::OpenAi => self.provider.openai.api_key = Some(key),
            ProviderKind::Anthropic => self.provider.anthropic.api_key = Some(key),
            ProviderKind::OpenRouter => self.provider.openrouter.api_key = Some(key),
            ProviderKind::NvidiaNim => self.provider.nvidianim.api_key = Some(key),
            ProviderKind::OpenCode => self.provider.opcode.api_key = Some(key),
            ProviderKind::Glm => self.provider.glm.api_key = Some(key),
            ProviderKind::Kimi => self.provider.kimi.api_key = Some(key),
            ProviderKind::KiloCode => self.provider.kilocode.api_key = Some(key),
            ProviderKind::Ollama => self.provider.ollama.api_key = Some(key),
            ProviderKind::Groq => self.provider.groq.api_key = Some(key),
            ProviderKind::Together => self.provider.together.api_key = Some(key),
            ProviderKind::Fireworks => self.provider.fireworks.api_key = Some(key),
            ProviderKind::DeepSeek => self.provider.deepseek.api_key = Some(key),
            ProviderKind::Cohere => self.provider.cohere.api_key = Some(key),
            ProviderKind::Generic => self.provider.nvidianim.api_key = Some(key),
        }
    }

    /// Editor command: `NCA_EDITOR`, then `[ui].editor`, then `EDITOR`, then `vim`.
    pub fn effective_editor_command(&self) -> String {
        if let Ok(v) = env::var("NCA_EDITOR") {
            let t = v.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
        if let Some(ref e) = self.ui.editor {
            let t = e.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
        env::var("EDITOR")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "vim".to_string())
    }

    pub fn sync_default_model_from_provider(&mut self) {
        self.model.default_model = self.provider.active_model().to_string();
    }

    /// Returns `true` if the first-run onboarding gate should be shown.
    /// Triggers when: onboarding not completed OR all API keys have been removed.
    pub fn needs_onboarding(&self) -> bool {
        !self.ui.onboarding_completed || !self.provider.any_api_key_present()
    }
}

/// User interface preferences persisted in config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Shell command to launch the external editor (e.g. `vim` or `code --wait`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<String>,
    /// Theme name (future: "default", "tokyonight", etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// Hide hint text in the composer area.
    #[serde(default)]
    pub hide_tips: bool,
    /// Lines per scroll event (default 3).
    #[serde(default = "default_scroll_speed")]
    pub scroll_speed: u16,
    /// Whether the user has completed the first-run onboarding flow.
    #[serde(default)]
    pub onboarding_completed: bool,
}

fn default_scroll_speed() -> u16 {
    3
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            editor: None,
            theme: None,
            hide_tips: false,
            scroll_speed: default_scroll_speed(),
            onboarding_completed: false,
        }
    }
}

impl UiConfig {
    fn merge(&mut self, partial: PartialUiConfig) {
        if let Some(editor) = partial.editor {
            self.editor = Some(editor);
        }
        if let Some(theme) = partial.theme {
            self.theme = Some(theme);
        }
        if let Some(hide_tips) = partial.hide_tips {
            self.hide_tips = hide_tips;
        }
        if let Some(scroll_speed) = partial.scroll_speed {
            self.scroll_speed = scroll_speed;
        }
        if let Some(onboarding_completed) = partial.onboarding_completed {
            self.onboarding_completed = onboarding_completed;
        }
    }
}

pub fn global_config_path() -> Option<PathBuf> {
    env::var_os("HOME").map(|home| PathBuf::from(home).join(".nca/config.toml"))
}

/// `$HOME/.nca` when `HOME` is set.
pub fn nca_home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(|home| PathBuf::from(home).join(".nca"))
}

/// Stable per-workspace id: `{slug}-{hex}` derived from the canonical workspace path.
pub fn workspace_cache_id(workspace_root: &Path) -> Result<(String, PathBuf), WorkspaceCacheError> {
    let canonical =
        workspace_root
            .canonicalize()
            .map_err(|source| WorkspaceCacheError::Canonicalize {
                path: workspace_root.to_path_buf(),
                source,
            })?;
    let path_str = canonical.to_string_lossy();
    let suffix = workspace_path_hash_suffix(path_str.as_ref());
    let slug = workspace_dir_slug(&canonical);
    Ok((format!("{slug}-{suffix}"), canonical))
}

/// `~/.nca/workspaces/<workspace-id>/`
pub fn workspace_cache_dir(workspace_root: &Path) -> Result<PathBuf, WorkspaceCacheError> {
    let (id, _) = workspace_cache_id(workspace_root)?;
    let home = nca_home_dir().ok_or(WorkspaceCacheError::NoHomeDir)?;
    Ok(home.join("workspaces").join(id))
}

/// Cached CLI index JSON for this workspace.
pub fn workspace_cli_index_path(workspace_root: &Path) -> Result<PathBuf, WorkspaceCacheError> {
    Ok(workspace_cache_dir(workspace_root)?.join("cli-index.json"))
}

fn workspace_dir_slug(path: &Path) -> String {
    let raw = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("workspace")
        .to_ascii_lowercase();
    let mut out = String::new();
    let mut prev_sep = false;
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_sep = false;
        } else if !out.is_empty() && !prev_sep {
            out.push('-');
            prev_sep = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "workspace".to_string()
    } else {
        trimmed
    }
}

fn workspace_path_hash_suffix(canonical_path: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(canonical_path.as_bytes());
    let digest = hasher.finalize();
    // 16 hex chars — stable across Rust versions (unlike std::collections::hash_map::DefaultHasher).
    format!("{digest:x}")[..16].to_string()
}

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceCacheError {
    #[error("HOME is not set")]
    NoHomeDir,
    #[error("failed to canonicalize workspace path {path}: {source}")]
    Canonicalize {
        path: PathBuf,
        source: std::io::Error,
    },
}

pub fn workspace_config_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(".nca").join("config.local.toml")
}

fn load_partial(path: &Path) -> Result<PartialNcaConfig, ConfigError> {
    let raw = std::fs::read_to_string(path).map_err(|source| ConfigError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    toml::from_str(&raw).map_err(|source| ConfigError::ParseToml {
        path: path.to_path_buf(),
        source,
    })
}

fn save_config_to_path(config: &NcaConfig, path: &Path) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ConfigError::Io {
            action: "create config directory",
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let raw = toml::to_string_pretty(config).map_err(|source| ConfigError::SerializeToml {
        path: path.to_path_buf(),
        source,
    })?;

    std::fs::write(path, raw).map_err(|source| ConfigError::Io {
        action: "write config file",
        path: path.to_path_buf(),
        source,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("unable to determine the home directory for global config")]
    NoHomeDir,
    #[error("failed to read config file {path}: {source}")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse config file {path}: {source}")]
    ParseToml {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("failed to serialize config file {path}: {source}")]
    SerializeToml {
        path: PathBuf,
        source: toml::ser::Error,
    },
    #[error("failed to {action} at {path}: {source}")]
    Io {
        action: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub default: ProviderKind,
    pub minimax: MiniMaxConfig,
    pub openai: OpenAiConfig,
    pub anthropic: AnthropicConfig,
    pub openrouter: OpenRouterConfig,
    pub nvidianim: GenericProviderConfig,
    pub opcode: GenericProviderConfig,
    pub glm: GenericProviderConfig,
    pub kimi: GenericProviderConfig,
    pub kilocode: GenericProviderConfig,
    pub ollama: GenericProviderConfig,
    pub ollamacloud: GenericProviderConfig,
    pub groq: GenericProviderConfig,
    pub together: GenericProviderConfig,
    pub fireworks: GenericProviderConfig,
    pub deepseek: GenericProviderConfig,
    pub cohere: GenericProviderConfig,
    pub sambanova: GenericProviderConfig,
    pub replicate: GenericProviderConfig,
    pub anyscale: GenericProviderConfig,
    pub perplexity: GenericProviderConfig,
    pub mistral: GenericProviderConfig,
    pub ai21: GenericProviderConfig,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            default: ProviderKind::MiniMax,
            minimax: MiniMaxConfig::default(),
            openai: OpenAiConfig::default(),
            anthropic: AnthropicConfig::default(),
            openrouter: OpenRouterConfig::default(),
            nvidianim: GenericProviderConfig::nvidia_nim(),
            opcode: GenericProviderConfig::opencode(),
            glm: GenericProviderConfig::glm(),
            kimi: GenericProviderConfig::kimi(),
            kilocode: GenericProviderConfig::kilocode(),
            ollama: GenericProviderConfig::ollama(),
            ollamacloud: GenericProviderConfig::ollama_cloud(),
            groq: GenericProviderConfig::groq(),
            together: GenericProviderConfig::together(),
            fireworks: GenericProviderConfig::fireworks(),
            deepseek: GenericProviderConfig::deepseek(),
            cohere: GenericProviderConfig::cohere(),
            sambanova: GenericProviderConfig::sambanova(),
            replicate: GenericProviderConfig::replicate(),
            anyscale: GenericProviderConfig::anyscale(),
            perplexity: GenericProviderConfig::perplexity(),
            mistral: GenericProviderConfig::mistral(),
            ai21: GenericProviderConfig::ai21(),
        }
    }
}

impl ProviderConfig {
    fn merge(&mut self, partial: PartialProviderConfig) {
        if let Some(default) = partial.default {
            self.default = default;
        }

        if let Some(minimax) = partial.minimax {
            self.minimax.merge(minimax);
        }
        if let Some(openai) = partial.openai {
            self.openai.merge(openai);
        }
        if let Some(anthropic) = partial.anthropic {
            self.anthropic.merge(anthropic);
        }
        if let Some(openrouter) = partial.openrouter {
            self.openrouter.merge(openrouter);
        }
        if let Some(nvidianim) = partial.nvidianim {
            self.nvidianim.merge(nvidianim);
        }
        if let Some(opcode) = partial.opcode {
            self.opcode.merge(opcode);
        }
        if let Some(glm) = partial.glm {
            self.glm.merge(glm);
        }
        if let Some(kimi) = partial.kimi {
            self.kimi.merge(kimi);
        }
        if let Some(kilocode) = partial.kilocode {
            self.kilocode.merge(kilocode);
        }
        if let Some(ollama) = partial.ollama {
            self.ollama.merge(ollama);
        }
        if let Some(ollamacloud) = partial.ollamacloud {
            self.ollamacloud.merge(ollamacloud);
        }
        if let Some(groq) = partial.groq {
            self.groq.merge(groq);
        }
        if let Some(together) = partial.together {
            self.together.merge(together);
        }
        if let Some(fireworks) = partial.fireworks {
            self.fireworks.merge(fireworks);
        }
        if let Some(deepseek) = partial.deepseek {
            self.deepseek.merge(deepseek);
        }
        if let Some(cohere) = partial.cohere {
            self.cohere.merge(cohere);
        }
        if let Some(sambanova) = partial.sambanova {
            self.sambanova.merge(sambanova);
        }
        if let Some(replicate) = partial.replicate {
            self.replicate.merge(replicate);
        }
        if let Some(anyscale) = partial.anyscale {
            self.anyscale.merge(anyscale);
        }
        if let Some(perplexity) = partial.perplexity {
            self.perplexity.merge(perplexity);
        }
        if let Some(mistral) = partial.mistral {
            self.mistral.merge(mistral);
        }
        if let Some(ai21) = partial.ai21 {
            self.ai21.merge(ai21);
        }
    }

    pub fn active_model(&self) -> &str {
        match self.default {
            ProviderKind::MiniMax => &self.minimax.model,
            ProviderKind::OpenRouter => &self.openrouter.model,
            ProviderKind::Anthropic => &self.anthropic.model,
            ProviderKind::OpenAi => &self.openai.model,
            ProviderKind::NvidiaNim => &self.nvidianim.model,
            ProviderKind::OpenCode => &self.opcode.model,
            ProviderKind::Glm => &self.glm.model,
            ProviderKind::Kimi => &self.kimi.model,
            ProviderKind::KiloCode => &self.kilocode.model,
            ProviderKind::Ollama => &self.ollama.model,
            ProviderKind::OllamaCloud => &self.ollamacloud.model,
            ProviderKind::Groq => &self.groq.model,
            ProviderKind::Together => &self.together.model,
            ProviderKind::Fireworks => &self.fireworks.model,
            ProviderKind::DeepSeek => &self.deepseek.model,
            ProviderKind::Cohere => &self.cohere.model,
            ProviderKind::SambaNova => &self.sambanova.model,
            ProviderKind::Replicate => &self.replicate.model,
            ProviderKind::Anyscale => &self.anyscale.model,
            ProviderKind::Perplexity => &self.perplexity.model,
            ProviderKind::Mistral => &self.mistral.model,
            ProviderKind::Ai21 => &self.ai21.model,
            ProviderKind::Generic => &self.nvidianim.model,
        }
    }

    pub fn set_model_for_default(&mut self, model: impl Into<String>) {
        self.set_model_for(self.default, model);
    }

    pub fn set_model_for(&mut self, provider: ProviderKind, model: impl Into<String>) {
        let model = model.into();
        match provider {
            ProviderKind::MiniMax => self.minimax.model = model,
            ProviderKind::OpenRouter => self.openrouter.model = model,
            ProviderKind::Anthropic => self.anthropic.model = model,
            ProviderKind::OpenAi => self.openai.model = model,
            ProviderKind::NvidiaNim => self.nvidianim.model = model,
            ProviderKind::OpenCode => self.opcode.model = model,
            ProviderKind::Glm => self.glm.model = model,
            ProviderKind::Kimi => self.kimi.model = model,
            ProviderKind::KiloCode => self.kilocode.model = model,
            ProviderKind::Ollama => self.ollama.model = model,
            ProviderKind::OllamaCloud => self.ollamacloud.model = model,
            ProviderKind::Groq => self.groq.model = model,
            ProviderKind::Together => self.together.model = model,
            ProviderKind::Fireworks => self.fireworks.model = model,
            ProviderKind::DeepSeek => self.deepseek.model = model,
            ProviderKind::Cohere => self.cohere.model = model,
            ProviderKind::SambaNova => self.sambanova.model = model,
            ProviderKind::Replicate => self.replicate.model = model,
            ProviderKind::Anyscale => self.anyscale.model = model,
            ProviderKind::Perplexity => self.perplexity.model = model,
            ProviderKind::Mistral => self.mistral.model = model,
            ProviderKind::Ai21 => self.ai21.model = model,
            ProviderKind::Generic => self.nvidianim.model = model,
        }
    }

    pub fn model_for(&self, provider: ProviderKind) -> &str {
        match provider {
            ProviderKind::MiniMax => &self.minimax.model,
            ProviderKind::OpenRouter => &self.openrouter.model,
            ProviderKind::Anthropic => &self.anthropic.model,
            ProviderKind::OpenAi => &self.openai.model,
            ProviderKind::NvidiaNim => &self.nvidianim.model,
            ProviderKind::OpenCode => &self.opcode.model,
            ProviderKind::Glm => &self.glm.model,
            ProviderKind::Kimi => &self.kimi.model,
            ProviderKind::KiloCode => &self.kilocode.model,
            ProviderKind::Ollama => &self.ollama.model,
            ProviderKind::OllamaCloud => &self.ollamacloud.model,
            ProviderKind::Groq => &self.groq.model,
            ProviderKind::Together => &self.together.model,
            ProviderKind::Fireworks => &self.fireworks.model,
            ProviderKind::DeepSeek => &self.deepseek.model,
            ProviderKind::Cohere => &self.cohere.model,
            ProviderKind::SambaNova => &self.sambanova.model,
            ProviderKind::Replicate => &self.replicate.model,
            ProviderKind::Anyscale => &self.anyscale.model,
            ProviderKind::Perplexity => &self.perplexity.model,
            ProviderKind::Mistral => &self.mistral.model,
            ProviderKind::Ai21 => &self.ai21.model,
            ProviderKind::Generic => &self.nvidianim.model,
        }
    }

    pub fn base_url_for(&self, provider: ProviderKind) -> &str {
        match provider {
            ProviderKind::MiniMax => &self.minimax.base_url,
            ProviderKind::OpenRouter => &self.openrouter.base_url,
            ProviderKind::Anthropic => &self.anthropic.base_url,
            ProviderKind::OpenAi => &self.openai.base_url,
            ProviderKind::NvidiaNim => &self.nvidianim.base_url,
            ProviderKind::OpenCode => &self.opcode.base_url,
            ProviderKind::Glm => &self.glm.base_url,
            ProviderKind::Kimi => &self.kimi.base_url,
            ProviderKind::KiloCode => &self.kilocode.base_url,
            ProviderKind::Ollama => &self.ollama.base_url,
            ProviderKind::OllamaCloud => &self.ollamacloud.base_url,
            ProviderKind::Groq => &self.groq.base_url,
            ProviderKind::Together => &self.together.base_url,
            ProviderKind::Fireworks => &self.fireworks.base_url,
            ProviderKind::DeepSeek => &self.deepseek.base_url,
            ProviderKind::Cohere => &self.cohere.base_url,
            ProviderKind::SambaNova => &self.sambanova.base_url,
            ProviderKind::Replicate => &self.replicate.base_url,
            ProviderKind::Anyscale => &self.anyscale.base_url,
            ProviderKind::Perplexity => &self.perplexity.base_url,
            ProviderKind::Mistral => &self.mistral.base_url,
            ProviderKind::Ai21 => &self.ai21.base_url,
            ProviderKind::Generic => &self.nvidianim.base_url,
        }
    }

    pub fn api_key_env_for(&self, provider: ProviderKind) -> &str {
        match provider {
            ProviderKind::MiniMax => &self.minimax.api_key_env,
            ProviderKind::OpenRouter => &self.openrouter.api_key_env,
            ProviderKind::Anthropic => &self.anthropic.api_key_env,
            ProviderKind::OpenAi => &self.openai.api_key_env,
            ProviderKind::NvidiaNim => &self.nvidianim.api_key_env,
            ProviderKind::OpenCode => &self.opcode.api_key_env,
            ProviderKind::Glm => &self.glm.api_key_env,
            ProviderKind::Kimi => &self.kimi.api_key_env,
            ProviderKind::KiloCode => &self.kilocode.api_key_env,
            ProviderKind::Ollama => &self.ollama.api_key_env,
            ProviderKind::OllamaCloud => &self.ollamacloud.api_key_env,
            ProviderKind::Groq => &self.groq.api_key_env,
            ProviderKind::Together => &self.together.api_key_env,
            ProviderKind::Fireworks => &self.fireworks.api_key_env,
            ProviderKind::DeepSeek => &self.deepseek.api_key_env,
            ProviderKind::Cohere => &self.cohere.api_key_env,
            ProviderKind::SambaNova => &self.sambanova.api_key_env,
            ProviderKind::Replicate => &self.replicate.api_key_env,
            ProviderKind::Anyscale => &self.anyscale.api_key_env,
            ProviderKind::Perplexity => &self.perplexity.api_key_env,
            ProviderKind::Mistral => &self.mistral.api_key_env,
            ProviderKind::Ai21 => &self.ai21.api_key_env,
            ProviderKind::Generic => &self.nvidianim.api_key_env,
        }
    }

    pub fn api_key_present_for(&self, provider: ProviderKind) -> bool {
        match provider {
            ProviderKind::MiniMax => self.minimax.resolve_api_key().is_some(),
            ProviderKind::OpenRouter => self.openrouter.resolve_api_key().is_some(),
            ProviderKind::Anthropic => self.anthropic.resolve_api_key().is_some(),
            ProviderKind::OpenAi => self.openai.resolve_api_key().is_some(),
            ProviderKind::NvidiaNim => self.nvidianim.resolve_api_key().is_some(),
            ProviderKind::OpenCode => self.opcode.resolve_api_key().is_some(),
            ProviderKind::Glm => self.glm.resolve_api_key().is_some(),
            ProviderKind::Kimi => self.kimi.resolve_api_key().is_some(),
            ProviderKind::KiloCode => self.kilocode.resolve_api_key().is_some(),
            ProviderKind::Ollama => self.ollama.resolve_api_key().is_some(),
            ProviderKind::OllamaCloud => self.ollamacloud.resolve_api_key().is_some(),
            ProviderKind::Groq => self.groq.resolve_api_key().is_some(),
            ProviderKind::Together => self.together.resolve_api_key().is_some(),
            ProviderKind::Fireworks => self.fireworks.resolve_api_key().is_some(),
            ProviderKind::DeepSeek => self.deepseek.resolve_api_key().is_some(),
            ProviderKind::Cohere => self.cohere.resolve_api_key().is_some(),
            ProviderKind::SambaNova => self.sambanova.resolve_api_key().is_some(),
            ProviderKind::Replicate => self.replicate.resolve_api_key().is_some(),
            ProviderKind::Anyscale => self.anyscale.resolve_api_key().is_some(),
            ProviderKind::Perplexity => self.perplexity.resolve_api_key().is_some(),
            ProviderKind::Mistral => self.mistral.resolve_api_key().is_some(),
            ProviderKind::Ai21 => self.ai21.resolve_api_key().is_some(),
            ProviderKind::Generic => self.nvidianim.resolve_api_key().is_some(),
        }
    }

    /// Returns `true` if at least one provider has an API key configured
    /// (either in config or via environment variable).
    pub fn any_api_key_present(&self) -> bool {
        ProviderKind::ALL
            .iter()
            .any(|p| self.api_key_present_for(*p))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    MiniMax,
    OpenRouter,
    Anthropic,
    OpenAi,
    NvidiaNim,
    OpenCode,
    Glm,
    Kimi,
    KiloCode,
    Ollama,
    OllamaCloud,
    Groq,
    Together,
    Fireworks,
    DeepSeek,
    Cohere,
    SambaNova,
    Replicate,
    Anyscale,
    Perplexity,
    Mistral,
    Ai21,
    Generic,
}

impl ProviderKind {
    pub const ALL: [ProviderKind; 23] = [
        ProviderKind::MiniMax,
        ProviderKind::OpenAi,
        ProviderKind::Anthropic,
        ProviderKind::OpenRouter,
        ProviderKind::NvidiaNim,
        ProviderKind::OpenCode,
        ProviderKind::Glm,
        ProviderKind::Kimi,
        ProviderKind::KiloCode,
        ProviderKind::Ollama,
        ProviderKind::OllamaCloud,
        ProviderKind::Groq,
        ProviderKind::Together,
        ProviderKind::Fireworks,
        ProviderKind::DeepSeek,
        ProviderKind::Cohere,
        ProviderKind::SambaNova,
        ProviderKind::Replicate,
        ProviderKind::Anyscale,
        ProviderKind::Perplexity,
        ProviderKind::Mistral,
        ProviderKind::Ai21,
        ProviderKind::Generic,
    ];

    /// Parse user/CLI input (slash commands, TUI pickers).
    pub fn from_cli_name(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "minimax" | "mini-max" | "minimaxi" => Some(Self::MiniMax),
            "openai" | "open-ai" | "gpt" => Some(Self::OpenAi),
            "anthropic" | "claude" => Some(Self::Anthropic),
            "openrouter" | "open-router" => Some(Self::OpenRouter),
            "nvidia" | "nvidianim" | "nim" => Some(Self::NvidiaNim),
            "opencode" | "open-code" => Some(Self::OpenCode),
            "glm" | "zhipu" => Some(Self::Glm),
            "kimi" | "moonshot" => Some(Self::Kimi),
            "kilocode" | "kilo-code" => Some(Self::KiloCode),
            "ollama" => Some(Self::Ollama),
            "ollamacloud" | "ollama-cloud" => Some(Self::OllamaCloud),
            "groq" => Some(Self::Groq),
            "together" | "togetherai" | "together-ai" => Some(Self::Together),
            "fireworks" | "fireworksai" | "fireworks-ai" => Some(Self::Fireworks),
            "deepseek" => Some(Self::DeepSeek),
            "cohere" => Some(Self::Cohere),
            "sambanova" | "samba-nova" | "samba" => Some(Self::SambaNova),
            "replicate" => Some(Self::Replicate),
            "anyscale" => Some(Self::Anyscale),
            "perplexity" => Some(Self::Perplexity),
            "mistral" => Some(Self::Mistral),
            "ai21" | "ai-21" => Some(Self::Ai21),
            "generic" | "custom" => Some(Self::Generic),
            _ => None,
        }
    }

    fn from_env(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "openrouter" => Self::OpenRouter,
            "anthropic" => Self::Anthropic,
            "openai" => Self::OpenAi,
            "nvidia" | "nvidianim" | "nim" => Self::NvidiaNim,
            "opencode" | "open-code" => Self::OpenCode,
            "glm" | "zhipu" => Self::Glm,
            "kimi" | "moonshot" => Self::Kimi,
            "kilocode" | "kilo-code" => Self::KiloCode,
            "ollama" => Self::Ollama,
            "ollamacloud" | "ollama-cloud" => Self::OllamaCloud,
            "groq" => Self::Groq,
            "together" | "togetherai" | "together-ai" => Self::Together,
            "fireworks" | "fireworksai" | "fireworks-ai" => Self::Fireworks,
            "deepseek" => Self::DeepSeek,
            "cohere" => Self::Cohere,
            "sambanova" | "samba-nova" | "samba" => Self::SambaNova,
            "replicate" => Self::Replicate,
            "anyscale" => Self::Anyscale,
            "perplexity" => Self::Perplexity,
            "mistral" => Self::Mistral,
            "ai21" | "ai-21" => Self::Ai21,
            "generic" | "custom" => Self::Generic,
            _ => Self::MiniMax,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            ProviderKind::MiniMax => "MiniMax",
            ProviderKind::OpenRouter => "OpenRouter",
            ProviderKind::Anthropic => "Anthropic",
            ProviderKind::OpenAi => "OpenAI",
            ProviderKind::NvidiaNim => "NVIDIA NIM",
            ProviderKind::OpenCode => "OpenCode",
            ProviderKind::Glm => "GLM",
            ProviderKind::Kimi => "Kimi",
            ProviderKind::KiloCode => "KiloCode",
            ProviderKind::Ollama => "Ollama",
            ProviderKind::OllamaCloud => "Ollama Cloud",
            ProviderKind::Groq => "Groq",
            ProviderKind::Together => "Together AI",
            ProviderKind::Fireworks => "Fireworks AI",
            ProviderKind::DeepSeek => "DeepSeek",
            ProviderKind::Cohere => "Cohere",
            ProviderKind::SambaNova => "SambaNova",
            ProviderKind::Replicate => "Replicate",
            ProviderKind::Anyscale => "Anyscale",
            ProviderKind::Perplexity => "Perplexity",
            ProviderKind::Mistral => "Mistral AI",
            ProviderKind::Ai21 => "AI21",
            ProviderKind::Generic => "Generic",
        }
    }

    /// Match [`display_name`](Self::display_name) output (case-insensitive).
    pub fn parse_display_name(s: &str) -> Option<Self> {
        let t = s.trim();
        Self::ALL
            .into_iter()
            .find(|k| k.display_name().eq_ignore_ascii_case(t))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiniMaxConfig {
    pub api_key_env: String,
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub temperature: f32,
}

impl Default for MiniMaxConfig {
    fn default() -> Self {
        Self {
            api_key_env: "MINIMAX_API_KEY".into(),
            api_key: None,
            // Anthropic-compatible endpoint (recommended for agentic/coding use).
            // International: https://api.minimax.io/anthropic
            // China:         https://api.minimaxi.com/anthropic
            base_url: "https://api.minimax.io/anthropic".into(),
            model: "MiniMax-M2.5".into(),
            temperature: 0.7,
        }
    }
}

impl MiniMaxConfig {
    pub fn resolve_api_key(&self) -> Option<String> {
        resolve_api_key_value(&self.api_key, &self.api_key_env)
    }

    fn merge(&mut self, partial: PartialMiniMaxConfig) {
        if let Some(api_key_env) = partial.api_key_env {
            self.api_key_env = api_key_env;
        }
        if let Some(api_key) = partial.api_key {
            self.api_key = Some(api_key);
        }
        if let Some(base_url) = partial.base_url {
            self.base_url = base_url;
        }
        if let Some(model) = partial.model {
            self.model = model;
        }
        if let Some(temperature) = partial.temperature {
            self.temperature = temperature;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiConfig {
    pub api_key_env: String,
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub temperature: f32,
}

impl Default for OpenAiConfig {
    fn default() -> Self {
        Self {
            api_key_env: "OPENAI_API_KEY".into(),
            api_key: None,
            base_url: "https://api.openai.com".into(),
            model: "gpt-4o-mini".into(),
            temperature: 0.7,
        }
    }
}

impl OpenAiConfig {
    pub fn resolve_api_key(&self) -> Option<String> {
        resolve_api_key_value(&self.api_key, &self.api_key_env)
    }

    fn merge(&mut self, partial: PartialOpenAiConfig) {
        if let Some(api_key_env) = partial.api_key_env {
            self.api_key_env = api_key_env;
        }
        if let Some(api_key) = partial.api_key {
            self.api_key = Some(api_key);
        }
        if let Some(base_url) = partial.base_url {
            self.base_url = base_url;
        }
        if let Some(model) = partial.model {
            self.model = model;
        }
        if let Some(temperature) = partial.temperature {
            self.temperature = temperature;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicConfig {
    pub api_key_env: String,
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub temperature: f32,
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            api_key_env: "ANTHROPIC_API_KEY".into(),
            api_key: None,
            base_url: "https://api.anthropic.com".into(),
            model: "claude-3-7-sonnet-latest".into(),
            temperature: 1.0,
        }
    }
}

impl AnthropicConfig {
    pub fn resolve_api_key(&self) -> Option<String> {
        resolve_api_key_value(&self.api_key, &self.api_key_env)
    }

    fn merge(&mut self, partial: PartialAnthropicConfig) {
        if let Some(api_key_env) = partial.api_key_env {
            self.api_key_env = api_key_env;
        }
        if let Some(api_key) = partial.api_key {
            self.api_key = Some(api_key);
        }
        if let Some(base_url) = partial.base_url {
            self.base_url = base_url;
        }
        if let Some(model) = partial.model {
            self.model = model;
        }
        if let Some(temperature) = partial.temperature {
            self.temperature = temperature;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterConfig {
    pub api_key_env: String,
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub temperature: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_name: Option<String>,
}

impl Default for OpenRouterConfig {
    fn default() -> Self {
        Self {
            api_key_env: "OPENROUTER_API_KEY".into(),
            api_key: None,
            base_url: "https://openrouter.ai/api".into(),
            model: "openai/gpt-4o-mini".into(),
            temperature: 0.7,
            site_url: None,
            app_name: None,
        }
    }
}

impl OpenRouterConfig {
    pub fn resolve_api_key(&self) -> Option<String> {
        resolve_api_key_value(&self.api_key, &self.api_key_env)
    }

    fn merge(&mut self, partial: PartialOpenRouterConfig) {
        if let Some(api_key_env) = partial.api_key_env {
            self.api_key_env = api_key_env;
        }
        if let Some(api_key) = partial.api_key {
            self.api_key = Some(api_key);
        }
        if let Some(base_url) = partial.base_url {
            self.base_url = base_url;
        }
        if let Some(model) = partial.model {
            self.model = model;
        }
        if let Some(temperature) = partial.temperature {
            self.temperature = temperature;
        }
        if let Some(site_url) = partial.site_url {
            self.site_url = Some(site_url);
        }
        if let Some(app_name) = partial.app_name {
            self.app_name = Some(app_name);
        }
    }
}

/// Generic provider config for OpenAI-compatible APIs (NVIDIA NIM, GLM, Kimi, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericProviderConfig {
    pub api_key_env: String,
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub temperature: f32,
}

impl GenericProviderConfig {
    pub fn resolve_api_key(&self) -> Option<String> {
        resolve_api_key_value(&self.api_key, &self.api_key_env)
    }

    fn merge(&mut self, partial: PartialGenericProviderConfig) {
        if let Some(api_key_env) = partial.api_key_env {
            self.api_key_env = api_key_env;
        }
        if let Some(api_key) = partial.api_key {
            self.api_key = Some(api_key);
        }
        if let Some(base_url) = partial.base_url {
            self.base_url = base_url;
        }
        if let Some(model) = partial.model {
            self.model = model;
        }
        if let Some(temperature) = partial.temperature {
            self.temperature = temperature;
        }
    }

    pub fn nvidia_nim() -> Self {
        Self {
            api_key_env: "NVIDIA_API_KEY".into(),
            api_key: None,
            base_url: "https://integrate.api.nvidia.com/v1".into(),
            model: "meta/llama-3.1-405b-instruct".into(),
            temperature: 0.7,
        }
    }

    pub fn opencode() -> Self {
        Self {
            api_key_env: "OPENCODE_API_KEY".into(),
            api_key: None,
            base_url: "https://api.opencode.ai/v1".into(),
            model: "opencode/coder".into(),
            temperature: 0.7,
        }
    }

    pub fn glm() -> Self {
        Self {
            api_key_env: "GLM_API_KEY".into(),
            api_key: None,
            base_url: "https://open.bigmodel.cn/api/paas/v4".into(),
            model: "glm-4-plus".into(),
            temperature: 0.7,
        }
    }

    pub fn kimi() -> Self {
        Self {
            api_key_env: "KIMI_API_KEY".into(),
            api_key: None,
            base_url: "https://api.moonshot.cn/v1".into(),
            model: "moonshot-v1-8k".into(),
            temperature: 0.7,
        }
    }

    pub fn kilocode() -> Self {
        Self {
            api_key_env: "KILOCODE_API_KEY".into(),
            api_key: None,
            base_url: "https://api.kilocode.ai/v1".into(),
            model: "kilocode/kilo".into(),
            temperature: 0.7,
        }
    }

    pub fn ollama() -> Self {
        Self {
            api_key_env: "OLLAMA_API_KEY".into(),
            api_key: None,
            base_url: "http://localhost:11434/v1".into(),
            model: "llama3.2".into(),
            temperature: 0.7,
        }
    }

    pub fn groq() -> Self {
        Self {
            api_key_env: "GROQ_API_KEY".into(),
            api_key: None,
            base_url: "https://api.groq.com/openai/v1".into(),
            model: "llama-3.3-70b-versatile".into(),
            temperature: 0.7,
        }
    }

    pub fn together() -> Self {
        Self {
            api_key_env: "TOGETHER_API_KEY".into(),
            api_key: None,
            base_url: "https://api.together.xyz/v1".into(),
            model: "meta-llama/Llama-3.3-70B-Instruct-Turbo".into(),
            temperature: 0.7,
        }
    }

    pub fn fireworks() -> Self {
        Self {
            api_key_env: "FIREWORKS_API_KEY".into(),
            api_key: None,
            base_url: "https://api.fireworks.ai/inference/v1".into(),
            model: "accounts/fireworks/models/llama-v3p3-70b-instruct".into(),
            temperature: 0.7,
        }
    }

    pub fn deepseek() -> Self {
        Self {
            api_key_env: "DEEPSEEK_API_KEY".into(),
            api_key: None,
            base_url: "https://api.deepseek.com/v1".into(),
            model: "deepseek-chat".into(),
            temperature: 0.7,
        }
    }

    pub fn cohere() -> Self {
        Self {
            api_key_env: "COHERE_API_KEY".into(),
            api_key: None,
            base_url: "https://api.cohere.com/compatibility/v1".into(),
            model: "command-r-plus".into(),
            temperature: 0.7,
        }
    }

    pub fn ollama_cloud() -> Self {
        Self {
            api_key_env: "OLLAMA_CLOUD_API_KEY".into(),
            api_key: None,
            base_url: "https://ollama.cloud/v1".into(),
            model: "llama3.2".into(),
            temperature: 0.7,
        }
    }

    pub fn sambanova() -> Self {
        Self {
            api_key_env: "SAMBANOVA_API_KEY".into(),
            api_key: None,
            base_url: "https://api.sambanova.ai/v1".into(),
            model: "Meta-Llama-3.1-405B-Instruct".into(),
            temperature: 0.7,
        }
    }

    pub fn replicate() -> Self {
        Self {
            api_key_env: "REPLICATE_API_KEY".into(),
            api_key: None,
            base_url: "https://api.replicate.com/v1".into(),
            model: "meta/meta-llama-3-70b-instruct".into(),
            temperature: 0.7,
        }
    }

    pub fn anyscale() -> Self {
        Self {
            api_key_env: "ANYSCALE_API_KEY".into(),
            api_key: None,
            base_url: "https://api.endpoints.anyscale.com/v1".into(),
            model: "meta-llama/Meta-Llama-3-70B-Instruct".into(),
            temperature: 0.7,
        }
    }

    pub fn perplexity() -> Self {
        Self {
            api_key_env: "PERPLEXITY_API_KEY".into(),
            api_key: None,
            base_url: "https://api.perplexity.ai".into(),
            model: "llama-3.1-sonar-small-128k-online".into(),
            temperature: 0.7,
        }
    }

    pub fn mistral() -> Self {
        Self {
            api_key_env: "MISTRAL_API_KEY".into(),
            api_key: None,
            base_url: "https://api.mistral.ai/v1".into(),
            model: "mistral-large-latest".into(),
            temperature: 0.7,
        }
    }

    pub fn ai21() -> Self {
        Self {
            api_key_env: "AI21_API_KEY".into(),
            api_key: None,
            base_url: "https://api.ai21.com/studio/v1".into(),
            model: "jamba-1.5-large".into(),
            temperature: 0.7,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub default_model: String,
    pub max_tokens: u32,
    pub enable_thinking: bool,
    pub thinking_budget: u32,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub aliases: BTreeMap<String, String>,
    /// Last N used model names for F2 cycling.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_models: Vec<String>,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            default_model: "MiniMax-M2.5".into(),
            max_tokens: 8192,
            enable_thinking: false,
            thinking_budget: 5120,
            aliases: default_model_aliases(),
            recent_models: Vec::new(),
        }
    }
}

impl ModelConfig {
    fn merge(&mut self, partial: PartialModelConfig) {
        if let Some(default_model) = partial.default_model {
            self.default_model = default_model;
        }
        if let Some(max_tokens) = partial.max_tokens {
            self.max_tokens = max_tokens;
        }
        if let Some(enable_thinking) = partial.enable_thinking {
            self.enable_thinking = enable_thinking;
        }
        if let Some(thinking_budget) = partial.thinking_budget {
            self.thinking_budget = thinking_budget;
        }
        if let Some(aliases) = partial.aliases {
            self.aliases = aliases;
        }
        if let Some(recent_models) = partial.recent_models {
            self.recent_models = recent_models;
        }
    }

    pub fn resolve_alias(&self, raw: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return self.default_model.clone();
        }

        let lowered = trimmed.to_ascii_lowercase();
        self.aliases
            .get(&lowered)
            .cloned()
            .unwrap_or_else(|| trimmed.to_string())
    }

    /// Push a model name to the front of the recent list, deduplicating and capping at 8.
    pub fn track_recent_model(&mut self, model: &str) {
        self.recent_models.retain(|m| m != model);
        self.recent_models.insert(0, model.to_string());
        self.recent_models.truncate(8);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PermissionConfig {
    pub mode: PermissionMode,
    pub allow: Vec<String>,
    pub deny: Vec<String>,
    pub ask: Vec<String>,
}

impl PermissionConfig {
    fn merge(&mut self, partial: PartialPermissionConfig) {
        if let Some(mode) = partial.mode {
            self.mode = mode;
        }
        if let Some(allow) = partial.allow {
            self.allow = allow;
        }
        if let Some(deny) = partial.deny {
            self.deny = deny;
        }
        if let Some(ask) = partial.ask {
            self.ask = ask;
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionMode {
    #[default]
    Default,
    Plan,
    AcceptEdits,
    DontAsk,
    BypassPermissions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub history_dir: PathBuf,
    #[serde(alias = "max_turn_per_run")]
    pub max_turns_per_run: u32,
    pub max_tool_calls_per_turn: u32,
    pub checkpoint_interval: u32,
    /// File that stores the last active session ID for auto-resume.
    pub last_session_file: PathBuf,
    /// Auto-compact when switching away from a session.
    #[serde(default)]
    pub auto_compact_on_finish: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            history_dir: PathBuf::from(".nca/sessions"),
            max_turns_per_run: 128,
            max_tool_calls_per_turn: 200,
            checkpoint_interval: 5,
            last_session_file: PathBuf::from(".nca/.last_session"),
            auto_compact_on_finish: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessConfig {
    pub built_in_enabled: bool,
    pub project_instructions_path: PathBuf,
    pub local_instructions_path: PathBuf,
    pub skill_directories: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpConfig {
    #[serde(default)]
    pub expose_in_safe_mode: bool,
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub file_path: PathBuf,
    #[serde(default = "default_max_memory_notes")]
    pub max_notes: usize,
    #[serde(default)]
    pub auto_compact_on_finish: bool,
    /// Context management configuration.
    #[serde(default)]
    pub context: ContextConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    /// Target context window size (approximate tokens).
    /// Set to 0 for auto-detection based on model, or specify a custom value.
    /// Auto-detection uses known model context windows.
    #[serde(default)]
    pub context_window_target: usize,
    /// Use model-specific context window detection.
    /// When true, ignores context_window_target and auto-detects from model name.
    #[serde(default = "default_true")]
    pub auto_detect_context_window: bool,
    /// When true with `auto_detect_context_window`, query the active provider's models API
    /// before falling back to built-in tables. OpenRouter's catalog is public; OpenAI and
    /// Anthropic require configured API keys. Set `NCA_SKIP_CONTEXT_API=1` to disable at runtime.
    /// Catalog responses are cached in-process; override TTL with `NCA_CONTEXT_API_CACHE_TTL_SECS`.
    #[serde(default = "default_true")]
    pub query_provider_models_api: bool,
    /// Maximum messages to retain after compaction.
    #[serde(default = "default_max_retained_messages")]
    pub max_retained_messages: usize,
    /// Percentage of context window that triggers auto-summarize (0-100).
    #[serde(default = "default_summarize_threshold")]
    pub auto_summarize_threshold: u8,
    /// Enable automatic context summarization.
    #[serde(default = "default_true")]
    pub enable_auto_summarize: bool,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            context_window_target: 0, // 0 means auto-detect
            auto_detect_context_window: true,
            query_provider_models_api: true,
            max_retained_messages: default_max_retained_messages(),
            auto_summarize_threshold: default_summarize_threshold(),
            enable_auto_summarize: default_true(),
        }
    }
}

fn default_summarize_threshold() -> u8 {
    75
}

fn default_max_retained_messages() -> usize {
    50
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HookConfig {
    #[serde(default)]
    pub session_start: Vec<HookCommand>,
    #[serde(default)]
    pub session_end: Vec<HookCommand>,
    #[serde(default)]
    pub pre_tool_use: Vec<HookCommand>,
    #[serde(default)]
    pub post_tool_use: Vec<HookCommand>,
    #[serde(default)]
    pub post_tool_failure: Vec<HookCommand>,
    #[serde(default)]
    pub approval_requested: Vec<HookCommand>,
    #[serde(default)]
    pub subagent_start: Vec<HookCommand>,
    #[serde(default)]
    pub subagent_stop: Vec<HookCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookCommand {
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    #[serde(default)]
    pub blocking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfig {
    pub timeout_secs: u64,
    pub max_fetch_chars: usize,
    pub default_search_limit: usize,
    pub user_agent: String,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 15,
            max_fetch_chars: 25_000,
            default_search_limit: 5,
            user_agent: "nca/0.5 (+https://github.com/user/native-cli-ai)".into(),
        }
    }
}

impl WebConfig {
    fn merge(&mut self, partial: PartialWebConfig) {
        if let Some(timeout_secs) = partial.timeout_secs {
            self.timeout_secs = timeout_secs;
        }
        if let Some(max_fetch_chars) = partial.max_fetch_chars {
            self.max_fetch_chars = max_fetch_chars;
        }
        if let Some(default_search_limit) = partial.default_search_limit {
            self.default_search_limit = default_search_limit;
        }
        if let Some(user_agent) = partial.user_agent {
            self.user_agent = user_agent;
        }
    }
}

impl Default for HarnessConfig {
    fn default() -> Self {
        Self {
            built_in_enabled: true,
            project_instructions_path: PathBuf::from(".ncarc"),
            local_instructions_path: PathBuf::from(".nca/instructions.md"),
            skill_directories: default_skill_directories(),
        }
    }
}

impl HarnessConfig {
    fn merge(&mut self, partial: PartialHarnessConfig) {
        if let Some(enabled) = partial.built_in_enabled {
            self.built_in_enabled = enabled;
        }
        if let Some(path) = partial.project_instructions_path {
            self.project_instructions_path = path;
        }
        if let Some(path) = partial.local_instructions_path {
            self.local_instructions_path = path;
        }
        if let Some(skill_directories) = partial.skill_directories {
            self.skill_directories = skill_directories;
        }
    }
}

impl McpConfig {
    fn merge(&mut self, partial: PartialMcpConfig) {
        if let Some(expose_in_safe_mode) = partial.expose_in_safe_mode {
            self.expose_in_safe_mode = expose_in_safe_mode;
        }
        if let Some(servers) = partial.servers {
            self.servers = servers;
        }
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            file_path: PathBuf::from(".nca/memory.json"),
            max_notes: default_max_memory_notes(),
            auto_compact_on_finish: false,
            context: ContextConfig::default(),
        }
    }
}

impl MemoryConfig {
    fn merge(&mut self, partial: PartialMemoryConfig) {
        if let Some(file_path) = partial.file_path {
            self.file_path = file_path;
        }
        if let Some(max_notes) = partial.max_notes {
            self.max_notes = max_notes;
        }
        if let Some(auto_compact_on_finish) = partial.auto_compact_on_finish {
            self.auto_compact_on_finish = auto_compact_on_finish;
        }
        if let Some(context) = partial.context {
            self.context.merge(context);
        }
    }
}

impl ContextConfig {
    fn merge(&mut self, partial: PartialContextConfig) {
        if let Some(auto_detect) = partial.auto_detect_context_window {
            self.auto_detect_context_window = auto_detect;
        }
        if let Some(context_window_target) = partial.context_window_target {
            self.context_window_target = context_window_target;
        }
        if let Some(max_retained_messages) = partial.max_retained_messages {
            self.max_retained_messages = max_retained_messages;
        }
        if let Some(auto_summarize_threshold) = partial.auto_summarize_threshold {
            self.auto_summarize_threshold = auto_summarize_threshold;
        }
        if let Some(enable_auto_summarize) = partial.enable_auto_summarize {
            self.enable_auto_summarize = enable_auto_summarize;
        }
        if let Some(query_provider_models_api) = partial.query_provider_models_api {
            self.query_provider_models_api = query_provider_models_api;
        }
    }
}

impl HookConfig {
    fn merge(&mut self, partial: PartialHookConfig) {
        if let Some(session_start) = partial.session_start {
            self.session_start = session_start;
        }
        if let Some(session_end) = partial.session_end {
            self.session_end = session_end;
        }
        if let Some(pre_tool_use) = partial.pre_tool_use {
            self.pre_tool_use = pre_tool_use;
        }
        if let Some(post_tool_use) = partial.post_tool_use {
            self.post_tool_use = post_tool_use;
        }
        if let Some(post_tool_failure) = partial.post_tool_failure {
            self.post_tool_failure = post_tool_failure;
        }
        if let Some(approval_requested) = partial.approval_requested {
            self.approval_requested = approval_requested;
        }
        if let Some(subagent_start) = partial.subagent_start {
            self.subagent_start = subagent_start;
        }
        if let Some(subagent_stop) = partial.subagent_stop {
            self.subagent_stop = subagent_stop;
        }
    }
}

impl SessionConfig {
    fn merge(&mut self, partial: PartialSessionConfig) {
        if let Some(history_dir) = partial.history_dir {
            self.history_dir = history_dir;
        }
        if let Some(max_turns_per_run) = partial.max_turns_per_run {
            self.max_turns_per_run = max_turns_per_run;
        }
        if let Some(max_tool_calls_per_turn) = partial.max_tool_calls_per_turn {
            self.max_tool_calls_per_turn = max_tool_calls_per_turn;
        }
        if let Some(checkpoint_interval) = partial.checkpoint_interval {
            self.checkpoint_interval = checkpoint_interval;
        }
        if let Some(last_session_file) = partial.last_session_file {
            self.last_session_file = last_session_file;
        }
        if let Some(auto_compact_on_finish) = partial.auto_compact_on_finish {
            self.auto_compact_on_finish = auto_compact_on_finish;
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PartialNcaConfig {
    provider: Option<PartialProviderConfig>,
    model: Option<PartialModelConfig>,
    permissions: Option<PartialPermissionConfig>,
    session: Option<PartialSessionConfig>,
    harness: Option<PartialHarnessConfig>,
    mcp: Option<PartialMcpConfig>,
    memory: Option<PartialMemoryConfig>,
    hooks: Option<PartialHookConfig>,
    web: Option<PartialWebConfig>,
    ui: Option<PartialUiConfig>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PartialUiConfig {
    editor: Option<String>,
    theme: Option<String>,
    hide_tips: Option<bool>,
    scroll_speed: Option<u16>,
    onboarding_completed: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PartialProviderConfig {
    default: Option<ProviderKind>,
    minimax: Option<PartialMiniMaxConfig>,
    openai: Option<PartialOpenAiConfig>,
    anthropic: Option<PartialAnthropicConfig>,
    openrouter: Option<PartialOpenRouterConfig>,
    nvidianim: Option<PartialGenericProviderConfig>,
    opcode: Option<PartialGenericProviderConfig>,
    glm: Option<PartialGenericProviderConfig>,
    kimi: Option<PartialGenericProviderConfig>,
    kilocode: Option<PartialGenericProviderConfig>,
    ollama: Option<PartialGenericProviderConfig>,
    ollamacloud: Option<PartialGenericProviderConfig>,
    groq: Option<PartialGenericProviderConfig>,
    together: Option<PartialGenericProviderConfig>,
    fireworks: Option<PartialGenericProviderConfig>,
    deepseek: Option<PartialGenericProviderConfig>,
    cohere: Option<PartialGenericProviderConfig>,
    sambanova: Option<PartialGenericProviderConfig>,
    replicate: Option<PartialGenericProviderConfig>,
    anyscale: Option<PartialGenericProviderConfig>,
    perplexity: Option<PartialGenericProviderConfig>,
    mistral: Option<PartialGenericProviderConfig>,
    ai21: Option<PartialGenericProviderConfig>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PartialMiniMaxConfig {
    api_key_env: Option<String>,
    api_key: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    temperature: Option<f32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PartialOpenAiConfig {
    api_key_env: Option<String>,
    api_key: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    temperature: Option<f32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PartialAnthropicConfig {
    api_key_env: Option<String>,
    api_key: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    temperature: Option<f32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PartialOpenRouterConfig {
    api_key_env: Option<String>,
    api_key: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    temperature: Option<f32>,
    site_url: Option<String>,
    app_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PartialGenericProviderConfig {
    api_key_env: Option<String>,
    api_key: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    temperature: Option<f32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PartialModelConfig {
    default_model: Option<String>,
    max_tokens: Option<u32>,
    enable_thinking: Option<bool>,
    thinking_budget: Option<u32>,
    aliases: Option<BTreeMap<String, String>>,
    recent_models: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PartialPermissionConfig {
    mode: Option<PermissionMode>,
    allow: Option<Vec<String>>,
    deny: Option<Vec<String>>,
    ask: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PartialSessionConfig {
    history_dir: Option<PathBuf>,
    #[serde(alias = "max_turn_per_run")]
    max_turns_per_run: Option<u32>,
    max_tool_calls_per_turn: Option<u32>,
    checkpoint_interval: Option<u32>,
    last_session_file: Option<PathBuf>,
    auto_compact_on_finish: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PartialHarnessConfig {
    built_in_enabled: Option<bool>,
    project_instructions_path: Option<PathBuf>,
    local_instructions_path: Option<PathBuf>,
    skill_directories: Option<Vec<PathBuf>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PartialMcpConfig {
    expose_in_safe_mode: Option<bool>,
    servers: Option<Vec<McpServerConfig>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PartialMemoryConfig {
    file_path: Option<PathBuf>,
    max_notes: Option<usize>,
    auto_compact_on_finish: Option<bool>,
    context: Option<PartialContextConfig>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PartialContextConfig {
    context_window_target: Option<usize>,
    auto_detect_context_window: Option<bool>,
    query_provider_models_api: Option<bool>,
    max_retained_messages: Option<usize>,
    auto_summarize_threshold: Option<u8>,
    enable_auto_summarize: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PartialHookConfig {
    session_start: Option<Vec<HookCommand>>,
    session_end: Option<Vec<HookCommand>>,
    pre_tool_use: Option<Vec<HookCommand>>,
    post_tool_use: Option<Vec<HookCommand>>,
    post_tool_failure: Option<Vec<HookCommand>>,
    approval_requested: Option<Vec<HookCommand>>,
    subagent_start: Option<Vec<HookCommand>>,
    subagent_stop: Option<Vec<HookCommand>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PartialWebConfig {
    timeout_secs: Option<u64>,
    max_fetch_chars: Option<usize>,
    default_search_limit: Option<usize>,
    user_agent: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_max_memory_notes() -> usize {
    128
}

fn default_model_aliases() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("default".into(), "MiniMax-M2.5".into()),
        ("minimax".into(), "MiniMax-M2.5".into()),
        ("m2.5".into(), "MiniMax-M2.5".into()),
        ("coding".into(), "MiniMax-M2.5".into()),
        ("reasoning".into(), "MiniMax-M2.5".into()),
        ("openai".into(), "gpt-4o-mini".into()),
        ("gpt4o".into(), "gpt-4o".into()),
        ("gpt4omini".into(), "gpt-4o-mini".into()),
        ("claude".into(), "claude-3-7-sonnet-latest".into()),
        ("claude-sonnet".into(), "claude-3-7-sonnet-latest".into()),
        ("openrouter".into(), "openai/gpt-4o-mini".into()),
    ])
}

fn resolve_api_key_value(inline: &Option<String>, env_name: &str) -> Option<String> {
    inline
        .as_deref()
        .filter(|v| !v.trim().is_empty())
        .map(String::from)
        .or_else(|| env::var(env_name).ok())
        .filter(|v| !v.trim().is_empty())
}

fn default_skill_directories() -> Vec<PathBuf> {
    vec![
        PathBuf::from(".nca/skills"),
        PathBuf::from(".claude/skills"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_accepts_max_turn_per_run_typo_alias() {
        let raw = r#"
            [session]
            max_turn_per_run = 99
        "#;
        let partial: PartialNcaConfig = toml::from_str(raw).expect("parse");
        let session = partial.session.expect("session table");
        assert_eq!(session.max_turns_per_run, Some(99));
    }

    #[test]
    fn apply_model_override_updates_selected_provider_model() {
        let mut config = NcaConfig::default();
        config.provider.default = ProviderKind::OpenAi;
        config.sync_default_model_from_provider();

        config.apply_model_override("gpt4o");

        assert_eq!(config.provider.openai.model, "gpt-4o");
        assert_eq!(config.model.default_model, "gpt-4o");
        assert_eq!(config.provider.minimax.model, "MiniMax-M2.5");
    }

    #[test]
    fn apply_env_supports_openai_anthropic_and_openrouter() {
        let _guard = EnvGuard::set(&[
            ("NCA_DEFAULT_PROVIDER", Some("openrouter")),
            ("OPENAI_API_KEY", Some("openai-key")),
            ("OPENAI_MODEL", Some("gpt-4o")),
            ("ANTHROPIC_API_KEY", Some("anthropic-key")),
            ("ANTHROPIC_MODEL", Some("claude-3-7-sonnet-20250219")),
            ("OPENROUTER_API_KEY", Some("openrouter-key")),
            ("OPENROUTER_MODEL", Some("anthropic/claude-3.7-sonnet")),
            ("OPENROUTER_SITE_URL", Some("https://nca.test")),
            ("OPENROUTER_APP_NAME", Some("Native CLI AI")),
        ]);

        let mut config = NcaConfig::default();
        config.apply_env();

        assert_eq!(config.provider.default, ProviderKind::OpenRouter);
        assert_eq!(
            config.provider.openai.resolve_api_key().as_deref(),
            Some("openai-key")
        );
        assert_eq!(
            config.provider.anthropic.resolve_api_key().as_deref(),
            Some("anthropic-key")
        );
        assert_eq!(
            config.provider.openrouter.resolve_api_key().as_deref(),
            Some("openrouter-key")
        );
        assert_eq!(config.provider.openai.model, "gpt-4o");
        assert_eq!(
            config.provider.anthropic.model,
            "claude-3-7-sonnet-20250219"
        );
        assert_eq!(
            config.provider.openrouter.model,
            "anthropic/claude-3.7-sonnet"
        );
        assert_eq!(
            config.provider.openrouter.site_url.as_deref(),
            Some("https://nca.test")
        );
        assert_eq!(
            config.provider.openrouter.app_name.as_deref(),
            Some("Native CLI AI")
        );
        assert_eq!(config.model.default_model, "anthropic/claude-3.7-sonnet");
    }

    struct EnvGuard {
        previous: Vec<(String, Option<String>)>,
    }

    impl EnvGuard {
        fn set(vars: &[(&str, Option<&str>)]) -> Self {
            let mut previous = Vec::new();
            for (key, value) in vars {
                previous.push((key.to_string(), env::var(key).ok()));
                match value {
                    Some(value) => unsafe { env::set_var(key, value) },
                    None => unsafe { env::remove_var(key) },
                }
            }
            Self { previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in self.previous.drain(..) {
                match value {
                    Some(value) => unsafe { env::set_var(&key, value) },
                    None => unsafe { env::remove_var(&key) },
                }
            }
        }
    }

    #[test]
    fn workspace_cache_id_stable_for_same_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (id1, p1) = workspace_cache_id(dir.path()).expect("id");
        let (id2, p2) = workspace_cache_id(dir.path()).expect("id");
        assert_eq!(id1, id2);
        assert_eq!(p1, p2);
        assert!(id1.contains('-'));
        assert!(id1.len() > 16);
    }

    #[test]
    fn ui_editor_roundtrips_through_workspace_file() {
        let _guard = EnvGuard::set(&[("NCA_EDITOR", None), ("EDITOR", None)]);
        let dir = tempfile::tempdir().expect("tempdir");
        let mut config = NcaConfig::default();
        config.ui.editor = Some("vim".into());
        config.set_default_provider(ProviderKind::MiniMax);
        config.save_workspace_file(dir.path()).expect("save");

        let loaded = NcaConfig::load_for_workspace(dir.path()).expect("load");
        assert_eq!(loaded.ui.editor.as_deref(), Some("vim"));
        assert_eq!(loaded.effective_editor_command(), "vim");
    }

    #[test]
    fn provider_kind_from_cli_name() {
        assert_eq!(
            ProviderKind::from_cli_name("MINIMAX"),
            Some(ProviderKind::MiniMax)
        );
        assert_eq!(
            ProviderKind::from_cli_name("openai"),
            Some(ProviderKind::OpenAi)
        );
        assert_eq!(ProviderKind::from_cli_name("nope"), None);
    }

    #[test]
    fn onboarding_completed_defaults_to_false() {
        let config = NcaConfig::default();
        assert!(!config.ui.onboarding_completed);
    }

    #[test]
    fn onboarding_completed_merges_from_partial() {
        let mut config = NcaConfig::default();
        let toml_str = r#"
[ui]
onboarding_completed = true
"#;
        let partial: PartialNcaConfig = toml::from_str(toml_str).unwrap();
        config.merge(partial);
        assert!(config.ui.onboarding_completed);
    }

    #[test]
    fn any_api_key_present_returns_false_when_no_keys() {
        let config = config_without_env_keys();
        assert!(!config.provider.any_api_key_present());
    }

    #[test]
    fn any_api_key_present_returns_true_when_one_key_set() {
        let mut config = NcaConfig::default();
        config.provider.openai.api_key = Some("sk-test".into());
        assert!(config.provider.any_api_key_present());
    }

    /// Returns an NcaConfig with env var fallbacks disabled so tests don't
    /// pick up real API keys from the shell environment.
    fn config_without_env_keys() -> NcaConfig {
        let mut config = NcaConfig::default();
        config.provider.minimax.api_key_env = "__NCA_TEST_NONE__".into();
        config.provider.openai.api_key_env = "__NCA_TEST_NONE__".into();
        config.provider.anthropic.api_key_env = "__NCA_TEST_NONE__".into();
        config.provider.openrouter.api_key_env = "__NCA_TEST_NONE__".into();
        config
    }

    #[test]
    fn needs_onboarding_true_when_no_flag_and_no_keys() {
        let config = config_without_env_keys();
        assert!(config.needs_onboarding());
    }

    #[test]
    fn needs_onboarding_false_when_flag_set_and_key_present() {
        let mut config = NcaConfig::default();
        config.ui.onboarding_completed = true;
        config.provider.minimax.api_key = Some("test-key".into());
        assert!(!config.needs_onboarding());
    }

    #[test]
    fn needs_onboarding_true_when_flag_set_but_all_keys_removed() {
        let mut config = config_without_env_keys();
        config.ui.onboarding_completed = true;
        // no keys set — safety net triggers
        assert!(config.needs_onboarding());
    }

    #[test]
    fn needs_onboarding_true_when_key_present_but_flag_not_set() {
        let mut config = NcaConfig::default();
        config.provider.openai.api_key = Some("sk-test".into());
        // onboarding_completed is false
        assert!(config.needs_onboarding());
    }

    #[test]
    fn onboarding_roundtrip_through_toml() {
        let toml_str = r#"
[ui]
onboarding_completed = true

[provider.minimax]
api_key = "test-key"
"#;
        let partial: PartialNcaConfig = toml::from_str(toml_str).unwrap();
        let mut config = NcaConfig::default();
        config.merge(partial);
        assert!(!config.needs_onboarding());
    }

    #[test]
    fn onboarding_triggers_when_key_removed_after_completion() {
        let toml_str = r#"
[ui]
onboarding_completed = true
"#;
        let partial: PartialNcaConfig = toml::from_str(toml_str).unwrap();
        let mut config = config_without_env_keys();
        config.merge(partial);
        assert!(config.needs_onboarding());
    }
}
