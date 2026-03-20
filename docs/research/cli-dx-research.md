# NCA CLI DX Research Program

## Objective
Improve the NCA CLI's developer experience through better session management, auto-resume intelligence, and workspace awareness.

## Editable Files
- crates/cli/src/main.rs
- crates/runtime/src/supervisor.rs
- crates/runtime/src/session_store.rs
- crates/cli/src/repl.rs
- crates/cli/src/tui/

## Fixed Files
- crates/common/src/session.rs
- crates/common/src/config.rs

## Metric
- cmd: `cargo test --package nca-cli 2>&1`
- regex: (\d+)\s+passed
- goal: maximize

## Constraints
- Time budget: 600 seconds
- Must not break existing functionality
- Must pass all tests
- Must maintain backward compatibility with session format

## Research Questions

### 1. Smart Session Discovery
How can we improve session auto-selection beyond "most recent"?
- [ ] Add session preview with conversation snippets
- [ ] Add context-based session suggestion (project-aware)
- [ ] Add session tagging/labeling

### 2. Session Organization
How to help users find and manage sessions?
- [ ] Add session search by content/date/status
- [ ] Add session grouping by project
- [ ] Add auto-cleanup of stale sessions

### 3. Better UX on Startup
What feedback should users get?
- [ ] Show session summary on resume
- [ ] Show conversation history preview
- [ ] Show project context detection

### 4. Performance Improvements
How to make session loading faster?
- [ ] Lazy load conversation history
- [ ] Incremental session checkpoints
- [ ] Cache session metadata

## Running autoresearch (metric probe)

From the repo root:

```bash
nca autoresearch once docs/research/cli-dx-research.md
```

This runs the metric shell command once and prints the first `(\d+) passed` capture from `cargo test` output.
