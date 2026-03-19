# TODO

## Done

- Built the core Rust-native CLI agent loop with MiniMax as the first-class provider.
- Added session-based CLI commands: `run`, `spawn`, `sessions`, `resume`, `logs`, `attach`, `status`, and `cancel`.
- Added human-readable and NDJSON streaming output.
- Added permission modes: `default`, `plan`, `accept-edits`, `dont-ask`, and `bypass-permissions`.
- Added web research tools: `web_search` and `fetch_url`.
- Added fast local code-intelligence with `query_symbols`.
- Added richer file and workflow tools: `apply_patch`, `edit_file`, `write_file`, `create_directory`, `rename_path`, `move_path`, `copy_path`, `delete_path`, `git_status`, `git_diff`, and `run_validation`.
- Added persisted session metadata, token/cost tracking, and IPC socket support for live session control.
- Updated the docs for CLI usage, parity progress, and architecture.

## Next Up: Re-scope native desktop (optional)

The former `nca-monitor` crate was removed. Before rebuilding:

- Write a short PRD for the next desktop client (goals, non-goals, vs CLI).
- Decide stack (egui/eframe is still a good Rust-native default).
- Reuse existing **runtime IPC** (`IpcClient`) and session/orchestration persistence—no duplicate protocol.

Backlog ideas for a future app (not committed scope):

- Live session list; connect to session sockets for streaming `AgentEvent`s.
- Session detail: messages, tool calls, approvals, diffs, costs.
- Multi-session / reconnect after restart.

## CLI + runtime hardening

- Verify `spawn`, `status`, `attach`, and `cancel` under automation (`--json` / NDJSON).
- Normalize event schemas for any future UI client.
- Improve IPC error handling and reconnect behavior.

## Later

- Add richer session search and filtering in CLI.
- Tmux / multiplexer awareness (see Phase 3 in PRD).

