use nca_common::config::PermissionMode;
use serde::Deserialize;
use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: Option<String>,
    pub command: String,
    pub model: Option<String>,
    pub permission_mode: Option<PermissionMode>,
    pub context: SkillContextMode,
    pub directory: PathBuf,
    pub body: String,
    pub source: SkillSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillContextMode {
    Inline,
    Fork,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillSource {
    AgentsMd,
    FileSystem,
}

pub struct SkillCatalog;

impl SkillCatalog {
    pub fn discover(
        workspace_root: &Path,
        skill_directories: &[PathBuf],
    ) -> Result<Vec<Skill>, String> {
        let mut roots = Vec::new();
        if let Some(home) = env::var_os("HOME") {
            let home = PathBuf::from(home);
            roots.push(home.join(".nca/skills"));
            roots.push(home.join(".claude/skills"));
        }

        for dir in skill_directories {
            if dir.is_absolute() {
                roots.push(dir.clone());
            } else {
                roots.push(workspace_root.join(dir));
            }
        }

        let mut skills = Vec::new();

        // Parse AGENTS.md first (takes precedence on conflicts)
        if let Ok(agents_skills) = parse_agents_md(workspace_root) {
            skills.extend(agents_skills);
        }

        // Then add filesystem skills (skip if command already exists)
        for root in roots {
            if !root.exists() {
                continue;
            }
            let entries = std::fs::read_dir(&root)
                .map_err(|err| format!("failed to read skills dir {}: {err}", root.display()))?;
            for entry in entries {
                let entry = entry.map_err(|err| err.to_string())?;
                let path = entry.path();
                let skill_file = if path.is_dir() {
                    path.join("SKILL.md")
                } else if path.file_name().and_then(|name| name.to_str()) == Some("SKILL.md") {
                    path.clone()
                } else {
                    continue;
                };
                if !skill_file.exists() {
                    continue;
                }
                if let Ok(skill) = parse_skill_file(&skill_file) {
                    if !skills
                        .iter()
                        .any(|existing: &Skill| existing.command == skill.command)
                    {
                        skills.push(skill);
                    }
                }
            }
        }

        skills.sort_by(|left, right| left.command.cmp(&right.command));
        Ok(skills)
    }
}

impl Skill {
    pub fn summary_line(&self) -> String {
        let source_tag = match self.source {
            SkillSource::AgentsMd => " [AGENTS.md]",
            SkillSource::FileSystem => "",
        };
        match &self.description {
            Some(description) => format!("/{:<14} {}{}", self.command, description, source_tag),
            None => format!("/{:<14} {}{}", self.command, self.name, source_tag),
        }
    }

    pub fn prompt_for_task(&self, task: &str) -> String {
        let mut prompt = format!(
            "Use the skill `{}`.\n\nSkill instructions:\n{}\n",
            self.command,
            self.body.trim()
        );
        if !task.trim().is_empty() {
            prompt.push_str(&format!("\nTask:\n{}\n", task.trim()));
        }
        prompt
    }

    pub fn manifest_summary(&self) -> String {
        let description = self
            .description
            .as_deref()
            .unwrap_or("No description provided.");
        let model = self.model.as_deref().unwrap_or("inherit");
        let permission_mode = self
            .permission_mode
            .map(|mode| format!("{mode:?}"))
            .unwrap_or_else(|| "inherit".into());
        let source_tag = match self.source {
            SkillSource::AgentsMd => " [AGENTS.md]",
            SkillSource::FileSystem => "",
        };
        format!(
            "- /{}: {}{}\n  model={model} permission_mode={permission_mode} context={:?}",
            self.command, description, source_tag, self.context
        )
    }

    pub fn source_label(&self) -> &'static str {
        match self.source {
            SkillSource::AgentsMd => "agents-md",
            SkillSource::FileSystem => "filesystem",
        }
    }
}

fn parse_skill_file(path: &Path) -> Result<Skill, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let directory = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let file_stem = directory
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("skill")
        .to_string();

    let (frontmatter, body) = split_frontmatter(&raw)?;
    let command = frontmatter.command.clone().unwrap_or_else(|| {
        slugify(
            &frontmatter
                .name
                .clone()
                .unwrap_or_else(|| file_stem.clone()),
        )
    });
    Ok(Skill {
        name: frontmatter.name.unwrap_or(file_stem),
        description: frontmatter.description,
        command,
        model: frontmatter.model,
        permission_mode: frontmatter.permission_mode,
        context: frontmatter.context.unwrap_or(SkillContextMode::Inline),
        directory,
        body: body.trim().to_string(),
        source: SkillSource::FileSystem,
    })
}

fn split_frontmatter(raw: &str) -> Result<(SkillFrontmatter, String), String> {
    if let Some(rest) = raw.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            let yaml = &rest[..end];
            let body = &rest[end + 5..];
            let fm = serde_yaml::from_str::<SkillFrontmatter>(yaml)
                .map_err(|err| format!("failed to parse skill frontmatter: {err}"))?;
            return Ok((fm, body.to_string()));
        }
    }
    Ok((SkillFrontmatter::default(), raw.to_string()))
}

fn slugify(value: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

#[derive(Debug, Default, Deserialize)]
struct SkillFrontmatter {
    name: Option<String>,
    description: Option<String>,
    command: Option<String>,
    model: Option<String>,
    permission_mode: Option<PermissionMode>,
    context: Option<SkillContextMode>,
}

impl<'de> Deserialize<'de> for SkillContextMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.trim().to_ascii_lowercase().as_str() {
            "fork" => Ok(Self::Fork),
            _ => Ok(Self::Inline),
        }
    }
}

/// Parse AGENTS.md as a skill manifest.
/// Each `## <Heading>` becomes a skill, with optional frontmatter.
fn parse_agents_md(workspace_root: &Path) -> Result<Vec<Skill>, String> {
    let agents_path = workspace_root.join("AGENTS.md");
    if !agents_path.exists() {
        return Ok(Vec::new());
    }

    let raw = std::fs::read_to_string(&agents_path)
        .map_err(|err| format!("failed to read AGENTS.md: {err}"))?;

    let mut skills = Vec::new();
    let mut current_heading = String::new();
    let mut current_content = String::new();
    let mut in_frontmatter = false;
    let mut frontmatter_lines = Vec::new();

    for line in raw.lines() {
        if line.starts_with("## ") {
            // Save previous skill if exists
            if !current_heading.is_empty() {
                if let Some(skill) = build_skill_from_section(
                    &current_heading,
                    &frontmatter_lines.join("\n"),
                    &current_content.trim(),
                    workspace_root,
                ) {
                    skills.push(skill);
                }
                frontmatter_lines.clear();
                in_frontmatter = false;
            }
            current_heading = line.trim_start_matches("## ").trim().to_string();
            current_content.clear();
        } else if line.trim().is_empty() {
            // Empty line - if in frontmatter, stay in frontmatter; otherwise accumulate
            if !in_frontmatter {
                if !current_content.is_empty() || !current_heading.is_empty() {
                    current_content.push('\n');
                }
            }
        } else if line.starts_with("- ") && !in_frontmatter && frontmatter_lines.is_empty() {
            // First directive line - check if it's a skill frontmatter directive
            let trimmed = line.trim_start_matches("- ");
            if trimmed.starts_with("model=")
                || trimmed.starts_with("permission_mode=")
                || trimmed.starts_with("context=")
            {
                in_frontmatter = true;
                frontmatter_lines.push(trimmed);
            } else {
                // Not a frontmatter directive, accumulate as content
                current_content.push_str(line);
                current_content.push('\n');
            }
        } else if in_frontmatter {
            if line.starts_with("- ") {
                frontmatter_lines.push(line.trim_start_matches("- "));
            } else {
                // Non-directive line ends frontmatter
                in_frontmatter = false;
                current_content.push_str(line);
                current_content.push('\n');
            }
        } else {
            current_content.push_str(line);
            current_content.push('\n');
        }
    }

    // Don't forget the last section
    if !current_heading.is_empty() {
        if let Some(skill) = build_skill_from_section(
            &current_heading,
            &frontmatter_lines.join("\n"),
            &current_content.trim(),
            workspace_root,
        ) {
            skills.push(skill);
        }
    }

    Ok(skills)
}

fn build_skill_from_section(
    heading: &str,
    frontmatter: &str,
    body: &str,
    workspace_root: &Path,
) -> Option<Skill> {
    let command = slugify(heading);
    if command.is_empty() {
        return None;
    }

    let mut model = None;
    let mut permission_mode = None;
    let mut context = SkillContextMode::Inline;

    // Parse frontmatter lines: model=inherit permission_mode=inherit context=Inline
    // Each line may contain multiple directives separated by spaces
    for line in frontmatter.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Split by spaces to handle "model=inherit permission_mode=plan"
        for directive in line.split_whitespace() {
            if directive.starts_with("model=") {
                let val = directive.trim_start_matches("model=").trim();
                if val != "inherit" {
                    model = Some(val.to_string());
                }
            } else if directive.starts_with("permission_mode=") {
                let val = directive.trim_start_matches("permission_mode=").trim();
                permission_mode = parse_permission_mode_str(val);
            } else if directive.starts_with("context=") {
                let val = directive
                    .trim_start_matches("context=")
                    .trim()
                    .to_lowercase();
                if val == "fork" {
                    context = SkillContextMode::Fork;
                }
            }
        }
    }

    Some(Skill {
        name: heading.to_string(),
        description: Some(section_description(heading, body)),
        command,
        model,
        permission_mode,
        context,
        directory: workspace_root.to_path_buf(),
        body: body.to_string(),
        source: SkillSource::AgentsMd,
    })
}

fn section_description(heading: &str, body: &str) -> String {
    body.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| {
            line.strip_prefix("- ")
                .or_else(|| line.strip_prefix("* "))
                .unwrap_or(line)
                .trim()
                .to_string()
        })
        .unwrap_or_else(|| heading.to_string())
}

fn parse_permission_mode_str(raw: &str) -> Option<PermissionMode> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "plan" => Some(PermissionMode::Plan),
        "accept-edits" | "accept_edits" => Some(PermissionMode::AcceptEdits),
        "dont-ask" | "dont_ask" => Some(PermissionMode::DontAsk),
        "bypass-permissions" | "bypass_permissions" => Some(PermissionMode::BypassPermissions),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_skill_frontmatter_and_body() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("review");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: Review PR\ndescription: Review code changes\ncommand: review\nmodel: MiniMax-M2.5\npermission_mode: plan\ncontext: fork\n---\nInspect diffs first.\n",
        )
        .unwrap();

        let skill = parse_skill_file(&skill_dir.join("SKILL.md")).unwrap();
        assert_eq!(skill.command, "review");
        assert_eq!(skill.context, SkillContextMode::Fork);
        assert_eq!(skill.permission_mode, Some(PermissionMode::Plan));
        assert!(skill.body.contains("Inspect diffs"));
    }

    #[test]
    fn parses_agents_md_as_skills() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("AGENTS.md"),
            r#"## Learned User Preferences

- Always use Rust-native solutions

Some detailed content here.

## Drizzle ORM

- model=inherit permission_mode=inherit context=Inline

Type-safe SQL ORM for TypeScript with zero runtime overhead

## Shadcn UI

- model=inherit permission_mode=plan context=fork

Component management and styling...
"#,
        )
        .unwrap();

        let skills = parse_agents_md(dir.path()).unwrap();
        assert_eq!(skills.len(), 3);

        // Check "Learned User Preferences" became a skill
        let pref = skills
            .iter()
            .find(|s| s.command == "learned-user-preferences")
            .unwrap();
        assert_eq!(pref.source, SkillSource::AgentsMd);
        assert_eq!(pref.context, SkillContextMode::Inline);
        assert_eq!(
            pref.description.as_deref(),
            Some("Always use Rust-native solutions")
        );

        // Check Drizzle ORM skill
        let driz = skills.iter().find(|s| s.command == "drizzle-orm").unwrap();
        assert_eq!(driz.source, SkillSource::AgentsMd);
        assert_eq!(driz.context, SkillContextMode::Inline);
        assert!(driz.description.as_ref().unwrap().contains("TypeScript"));

        // Check Shadcn skill
        let shad = skills.iter().find(|s| s.command == "shadcn-ui").unwrap();
        assert_eq!(shad.context, SkillContextMode::Fork);
        assert_eq!(shad.permission_mode, Some(PermissionMode::Plan));
    }
}
