# nca

![nca hero](docs/images/nca-readme-hero.png)

`nca` is a Rust-native coding CLI that ships as a single binary. It is built for local-first, terminal-first workflows: interactive TUI, line REPL, one-shot runs, detached sessions, attach/status/logs, JSON and NDJSON output, Unix-socket IPC, and worktree-isolated subagents.

It is meant for people who like their AI tooling close to the terminal: fast to start, easy to script, and capable of running real session workflows without dragging in a browser shell.

The product surface is the CLI. No desktop wrapper, no Electron, no browser in the default path.

## What It Does

- Runs coding tasks in an interactive TUI or a line-oriented REPL.
- Supports one-shot runs and detached background sessions.
- Persists session state and event logs under the current workspace.
- Exposes machine-readable JSON and NDJSON for automation.
- Spawns child agents with explicit parent/child lineage and optional git worktrees.
- Uses MiniMax by default, with OpenAI, Anthropic, and OpenRouter support.
- Loads built-in tools plus optional MCP tools from config.
- Sends **native multimodal** (text + image) messages to MiniMax and other vision-capable models; images are stored under `.nca/sessions/<id>/attachments/`.

## Why People Reach For It

- ⚡ You want a coding CLI that feels quick and stays out of the way.
- 🧠 You want sessions, event logs, and resumable work instead of a throwaway prompt box.
- 🌿 You want child agents that can branch off cleanly with lineage and optional git worktrees.
- 🤖 You want a CLI that still works well when another system is driving it through JSON, NDJSON, and IPC.

## Common Use Cases

| Use case | Why `nca` fits |
|---|---|
| Solo coding in the terminal | Start with `nca`, use the TUI, switch agent profiles, review diffs, and keep everything in one terminal-native flow. |
| Quick one-shot work | `nca run --prompt ...` gives you a focused foreground task without opening a longer session than you need. |
| Background analysis | `nca spawn --prompt ...` lets you kick off work, keep coding, then come back with `status`, `logs`, or `attach`. |
| Multi-agent exploration | Parent and child sessions keep lineage, and child runs can use separate git worktrees for isolation. |
| Automation and orchestration | `--json`, `--stream ndjson`, Unix-socket IPC, and `NCA_ORCH_*` metadata make it usable as a worker process. |

## Quick Start

This workspace uses Rust edition 2024, so use a recent Rust toolchain.

```bash
# Build and install
cargo build --release
cp target/release/nca /usr/local/bin/

# Configure the default provider
export MINIMAX_API_KEY="your-api-key"

# Start the interactive CLI
nca

# Line REPL instead of the full-screen TUI
nca --no-tui

# Run one task and exit
nca run --prompt "Explain this repository"

# Spawn a detached session
nca spawn --prompt "Inspect the repo and draft a plan"

# Inspect and attach
nca sessions
nca status <session_id>
nca attach <session_id>
```

The full-screen UI appears when `stdin` and `stdout` are TTYs and `--stream human` is active. Otherwise `nca` falls back to the line-oriented REPL or one-shot execution path.

### Images (full-screen TUI)

In the default TUI you can attach images for the **next** user message:

- **Ctrl+V** — paste a bitmap from the system clipboard (saved as PNG under the session).
- **`/image paste`** — same as clipboard paste if Ctrl+V is not available.
- **`/image path/to/screenshot.png`** — copy a file into the session attachment dir.
- **`/image clear`** — remove staged images before you press Enter.

For **MiniMax**, pasted images are analyzed with the same HTTP API as the MCP’s `understand_image` tool (`POST /v1/coding_plan/vlm` on `https://api.minimax.io` or your region host—see [MiniMax-Coding-Plan-MCP](https://github.com/MiniMax-AI/MiniMax-Coding-Plan-MCP)); nca does this in Rust (no Python MCP). The description is merged into the user message before `/v1/messages`. Other providers use their own multimodal chat formats where supported. If the **selected provider/model is not treated as vision-capable**, `nca` **errors** instead of silently dropping images. Session attachment copies are removed automatically after a successful send/process; your original source file is not deleted.

## A Quick Look 👀

The main interface is designed to feel like a serious terminal tool, not a toy overlay.

![nca interactive view](docs/images/nca-show.png)

## Core Commands

| Command | Purpose |
|---|---|
| `nca` | Start the default interactive experience. Auto-resumes the last session unless `--no-resume` is used. |
| `nca run --prompt ...` | Run one task in the foreground. |
| `nca spawn --prompt ...` | Start a detached session and return immediately. |
| `nca resume <session_id>` | Resume a saved session. |
| `nca attach <session_id>` | Attach to a running session over IPC. |
| `nca logs <session_id>` | Read or follow the event log. |
| `nca status <session_id>` | Show session status and metadata. |
| `nca cancel <session_id>` | Mark a detached session as cancelled. |
| `nca sessions` | List saved sessions, with filters like `--status`, `--since-hours`, and `--search`. |
| `nca models` | Show configured models and provider-facing defaults. |
| `nca doctor` | Check provider readiness, skills, and memory/config paths. |
| `nca config` | Print effective config and resolved paths. |
| `nca memory list|add` | Inspect or append workspace memory notes. |
| `nca skills` | List discovered skills. |
| `nca mcp` | List configured MCP servers. |
| `nca completion <shell>` | Generate shell completions. |
| `nca index build|show` | Build or inspect a cached CLI index under `~/.nca/workspaces/<workspace-id>/`. |
| `nca autoresearch ...` | Experimental autonomous research helpers. |

There is also a hidden `serve` subcommand used for IPC-oriented service sessions.

## Interactive UX

The interactive surface has two modes:

- Full-screen TUI with transcript, composer, approvals, structured questions, slash-command palette, session sidebar, and branch picker.
- Line-oriented REPL built on `reedline` for scripts, terminals where TUI is not desired, or cases where `--no-tui` is easier.

Useful interactive behaviors:

- `! <cmd>` runs a shell command.
- `@ <query>` searches files.
- `/...` runs slash commands.
- `Tab` cycles agent profiles such as `build`, `plan`, `review`, `fix`, and `test`.
- `Ctrl+C` or `/stop` cancels the current running turn.
- `/auto-answer` accepts the suggested answer for a pending `ask_question`.

Small touches in the TUI matter too: branch switching, structured options, session sidebars, and direct control over long-running turns.

![branch picker](docs/images/git-branch.png)

![interactive options](docs/images/option.png)

## Output and Automation

`nca` is designed to work well in two very different moods: terminal-first for humans, and machine-friendly for orchestrators.

- `--stream off` returns only the final output.
- `--stream human` renders the normal terminal experience.
- `--stream ndjson` emits newline-delimited event envelopes.
- `--json` is available on lifecycle-oriented commands such as `spawn`, `sessions`, `status`, and `cancel`.
- `NCA_ORCH_*` and `NCA_ORCH_META_*` environment variables attach orchestration metadata to sessions and harness context.

See [Orchestration Contract](docs/orchestration.md) for the subprocess-facing surface.

## Storage and Paths

`nca` is workspace-first. The current workspace keeps its own session history and local state.

| Path | Purpose |
|---|---|
| `~/.nca/config.toml` | Global config file. |
| `<workspace>/.nca/config.local.toml` | Workspace-local config overrides. |
| `<workspace>/.nca/sessions/<id>.json` | Saved session state. |
| `<workspace>/.nca/sessions/<id>.events.jsonl` | Event log for the session. |
| `<workspace>/.nca/memory.json` | Default memory store. |
| `<workspace>/.nca/skills/` | Default workspace skill directory. |
| `<repo>/.nca/worktrees/<session-id>` | Worktree path for isolated child sessions. |
| `$XDG_RUNTIME_DIR/nca/<session_id>.sock` | IPC socket path when `XDG_RUNTIME_DIR` is set. |
| `/tmp/nca/<session_id>.sock` | IPC socket fallback when `XDG_RUNTIME_DIR` is not set. |
| `~/.nca/workspaces/<workspace-id>/cli-index.json` | Cached CLI index for agents and tooling. |
| `.ncarc` | Project instructions file committed with the repo. |
| `.nca/instructions.md` | Local instructions file. |

## Providers

MiniMax is the default provider path. The codebase also supports OpenAI, Anthropic, and OpenRouter, so the project can stay MiniMax-first without boxing itself into one provider forever.

Typical environment variables:

- `MINIMAX_API_KEY`
- `OPENAI_API_KEY`
- `ANTHROPIC_API_KEY`
- `OPENROUTER_API_KEY`

Provider config is loaded from defaults, then `~/.nca/config.toml`, then `<workspace>/.nca/config.local.toml`, then environment overrides.

Use `nca doctor` to verify provider readiness and `nca models` to inspect model selection.

## Harness and Tooling

The system prompt is layered in this order:

1. Built-in harness prompt
2. Permission-mode guidance
3. Project instructions from `.ncarc`
4. Local instructions from `.nca/instructions.md`
5. Discovered skills summary
6. Orchestration context

The built-in tool surface includes filesystem editing, search, diffing, patching, shell execution, web access, `ask_question`, and `spawn_subagent`. MCP tools are loaded dynamically when configured, so the available tool set can grow with your environment.

## Crate Layout

| Crate | Responsibility |
|---|---|
| `crates/common` | Shared config, events, sessions, messages, tool schemas, and orchestration metadata. |
| `crates/core` | Agent loop, provider abstraction, harness builder, skills, approvals, and tool registry. |
| `crates/runtime` | Session supervision, IPC, persistence, worktrees, memory store, and subagent execution. |
| `crates/cli` | `nca` entrypoint, command parsing, stream rendering, REPL, and TUI. |

## Session Model

- Sessions are persisted as JSON snapshots plus JSONL event logs.
- The runtime uses a `Supervisor` to own lifecycle, IPC, approvals, questions, event fanout, and persistence.
- Child sessions can inherit parent context, record lineage in session metadata, and run inside separate git worktrees.
- IPC uses newline-delimited JSON over Unix sockets so `attach`, approvals, status, and other controls share one runtime transport.

In practice, that means you can start small, branch out when a task gets bigger, and still keep a clean trail of what happened.

## Documentation

The root README is the quick-start guide. Use the docs folder for deeper detail:

- [Product Requirements](docs/prd.md)
- [Tech Stack](docs/tech-stack.md)
- [Architecture](docs/architecture.md)
- [Orchestration Contract](docs/orchestration.md)

## License

MIT
