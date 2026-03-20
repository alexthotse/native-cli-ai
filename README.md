# nca - Native CLI AI

![nca hero](docs/images/nca-readme-hero.png)

`nca` is a Rust-native coding CLI for people who want something fast, local, and easy to run at scale. It starts quickly, stays light on memory, and is built for workflows where you may want to run **many agents and sub-agents in parallel** instead of babysitting one heavy session at a time.

The focus here is simple: keep making the CLI better. Better harnessing, better spawning, better tooling, and better machine-readable surfaces. No JavaScript, no Electron, no browser in the default path. Just one `nca` binary with Unix-socket IPC and JSON/NDJSON when you need automation.

## Why nca

Most coding CLIs are optimized for one interactive session. `nca` is aimed more at **worker-style usage**: spawn a task, attach later, run child agents in worktrees, and keep the overhead low enough that parallel runs are actually practical.

| Why it matters | What you get |
|----------------|--------------|
| **Performance** | Fast startup and a tight Rust stack, so each new run stays cheap. |
| **Low memory use** | You can keep many sessions and child runs around without paying for a heavy UI/runtime on each one. |
| **Spawn-first workflows** | First-class `spawn`, `sessions`, `attach`, worktree-isolated **sub-agents**, and clear parent/child lineage. |
| **Automation-friendly** | `run` / `spawn` / `status` / `cancel` with `--json`, NDJSON event streams, and `NCA_ORCH_*` metadata for external control planes. |

This repo is **CLI-only**. If what you really want is a dashboard, a desktop shell, or higher-level agent orchestration, use a separate orchestration layer and let `nca` be the executor.

## Desktop mode & agent orchestration

If you want a **web dashboard**, multi-agent supervision, or a **desktop control plane**, this repository is not trying to do that itself. Instead, use an orchestration layer that can call `nca` through its [orchestration contract](docs/orchestration.md):

- **[Paperclip](https://github.com/paperclipai/paperclip)** — an orchestration layer for coordinating many agents, goals, approvals, and governance.  
- **[Enterprise Orchestration](https://github.com/madebyaris/enterprise-orchestration)** *(from us)* — a local-first desktop/control-plane project where **native-cli-ai** is a first-class executor target.

## Why Try It

- Rust-native stack from top to bottom.
- MiniMax-first by default, with compatibility for OpenAI, Anthropic/Claude, and OpenRouter.
- Headless-friendly JSON and NDJSON surfaces for orchestration systems.
- Background sessions, event logs, attachable runs, and isolated child-agent worktrees.
- JSON/NDJSON surfaces for automation and subprocess integration.

## Product Direction

We are leaning further into the CLI instead of backing away from it: stronger harness defaults, fail-loud provider behavior, a richer tool surface, and stable event schemas for people building wrappers around `nca`.

- Session lifecycle, events, and approvals all use the same runtime IPC (`attach`, approvals, shutdown).
- Interactive clarification uses the `ask_question` tool: structured options, optional custom text, always a model `suggested_answer`, plus `/auto-answer` in the REPL/TUI to accept the suggestion.
- Child sessions get **git worktrees** and explicit lineage so parallel agents do not step on each other.

If you are building a control plane, the intended split is straightforward: let `nca` stay the **lean worker process**, and let tools like Paperclip or Enterprise Orchestration handle dashboards, persistence, and fleet-level coordination.

## Quick Start

This workspace uses Rust edition 2024, so use a recent Rust toolchain first.

```bash
# Build release binaries
cargo build --release

# Install locally
cp target/release/nca /usr/local/bin/

# Configure MiniMax (default provider)
export MINIMAX_API_KEY="your-api-key"

# Start the interactive CLI (full-screen TUI when stdout+stdin are TTYs and --stream human)
nca
# Line-oriented REPL instead: scripts, piped stdin, or tricky tool-approval prompts
nca --no-tui

# Run a one-shot task
nca run --prompt "Explain this repository" --stream human

# Spawn a background worker
nca spawn --prompt "Inspect the repo and draft a plan"

# Inspect and attach
nca sessions
nca status <session_id>
nca attach <session_id>
```

The full-screen CLI cannot change your font from Rust—it follows your terminal’s monospace setting. For a modern look (and correct TUI alignment), see [CLI terminal fonts](docs/cli-terminal-fonts.md).

## Built For Agent Orchestration

`nca` is designed to be launched by other systems, wrappers, and local control planes.

- `nca run --stream off --json` returns a final structured result.
- `nca run --stream ndjson` streams live `EventEnvelope` updates.
- `nca spawn --json`, `status --json`, `sessions --json`, and `cancel --json` give machine-readable lifecycle control.
- `NCA_ORCH_*` environment variables let orchestrators inject run metadata into session state and the harness.
- Headless approval failures fail loudly instead of hanging forever.

Local persistence is workspace-first:

- `<workspace>/.nca/sessions/` for session snapshots and event logs
- `~/.nca/config.toml` (and optional `.nca/config.local.toml`) for global settings

See [Orchestration Contract](docs/orchestration.md) for the exact subprocess surface.

## Provider Story

- MiniMax is the default and recommended path.
- OpenAI, Anthropic/Claude, and OpenRouter are also supported.
- Config loads from `~/.nca/config.toml`, `.nca/config.local.toml`, and provider-specific environment variables.
- `nca doctor` and `nca models` expose provider readiness and active model selection.

Example provider environment variables:

- `MINIMAX_API_KEY`
- `OPENAI_API_KEY`
- `ANTHROPIC_API_KEY`
- `OPENROUTER_API_KEY`

## Harness Layers

The system prompt is layered so repo defaults stay strong without blocking team or local overrides.

1. Built-in harness prompt
2. Permission mode section
3. Project instructions from `.ncarc`
4. Local instructions from `.nca/instructions.md`
5. Discovered skills summary
6. Orchestration context

You can commit `.ncarc` for shared conventions and keep `.nca/instructions.md` local.

## CLI Surfaces

- Interactive REPL
- One-shot `run`
- Background `spawn`
- Session `resume`
- Event `logs`
- Live `attach`
- Per-session `status`
- `cancel` for spawned work
- Stream modes: `off`, `human`, `ndjson`
- Permission modes: `default`, `plan`, `accept-edits`, `dont-ask`, `bypass-permissions`

## Tools

Current tool-running path supports:

- `read_file`
- `search_code`
- `list_directory`
- `write_file`
- `create_directory`
- `git_status`
- `git_diff`
- `query_symbols`
- `web_search`
- `fetch_url`
- `apply_patch`
- `edit_file`
- `rename_path`
- `move_path`
- `copy_path`
- `delete_path`
- `run_validation`
- `execute_bash`

## Workspace Layout

| Crate | Description |
|-------|-------------|
| `crates/common` | Shared types, config, events |
| `crates/core` | Agent loop, provider abstraction, harness, tools |
| `crates/runtime` | IPC, session lifecycle, persistence, worktree/runtime glue |
| `crates/cli` | Terminal entrypoint and machine-facing control plane |

## Documentation

- [Product Requirements](docs/prd.md)
- [Tech Stack](docs/tech-stack.md)
- [Architecture](docs/architecture.md)
- [Orchestration Contract](docs/orchestration.md)

## License

MIT
