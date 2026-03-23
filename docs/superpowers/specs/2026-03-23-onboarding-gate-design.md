# First-Run Onboarding Gate

**Date:** 2026-03-23
**Status:** Approved

## Overview

When `nca` launches in TUI mode, it checks whether the user has a valid API key configured. If not, it force-opens the existing `/connect` modal in a "locked" mode and blocks chat input until the user provides and validates a working API key.

## Detection Logic

Two conditions trigger the onboarding gate:

1. **No `onboarding_completed` flag** — new field in `~/.nca/config.toml` under `[ui]`, defaults to `false`
2. **No API key present for any provider** — safety net so removing all keys re-triggers onboarding

If `onboarding_completed == false` OR no API key is present for any provider → gate activates.

A new `any_api_key_present()` helper on `ProviderConfig` checks all providers using the existing `api_key_present_for()` method.

## User Flow

```
nca launch
  → load config
  → check: onboarding_completed == true AND at least one API key present?
    → YES: normal chat
    → NO:  open /connect modal in "locked" mode
            → user picks provider
            → user enters API key (masked)
            → validate key against provider API
              → FAIL: show error message inline, stay in modal, user can retry or pick different provider
              → PASS: save key to global config (~/.nca/config.toml) via save_global(),
                      set onboarding_completed = true, close modal, start chat
```

## Locked Modal Behavior

- The `/connect` modal opens automatically on TUI init when the gate is triggered
- **Escape key is disabled** — the modal cannot be dismissed (guard on `onboarding_mode` before `close_connect_modal()`)
- Chat input area is **not focusable** while the modal is open
- The modal title remains `"Connect a Provider"` (same as current)
- All other modal functionality (search, provider categories, key input) works normally
- Reuse the existing `api_key_connect_after_save` flag to drive the "close modal and start chat" behavior

## API Key Validation

Per-provider lightweight validation calls:

| Provider   | Validation Method                                                  |
|------------|--------------------------------------------------------------------|
| Anthropic  | `POST /v1/messages` with minimal payload (or `GET /v1/models`)     |
| OpenAI     | `GET /v1/models`                                                   |
| MiniMax    | `POST` to Anthropic-compatible endpoint with minimal payload       |
| OpenRouter | `GET /v1/models`                                                   |

### Async Architecture

The TUI input loop (`run_blocking()`) is synchronous. Validation requires an async HTTP call. The approach:

1. After the user submits an API key, the TUI sends a new `TuiCmd::ValidateApiKey(ProviderKind, String)` command via `cmd_tx`
2. The async REPL side handles this command, performs the HTTP validation call
3. The result is communicated back to the TUI via the shared `Arc<Mutex<TuiSessionState>>` (e.g., setting a `validation_result` field)
4. The TUI polls this state on each render tick and reacts accordingly

### Error Handling

- **Timeout:** 10 seconds
- **Network error:** Show "Connection failed — check your network and try again"
- **401/403:** Show "Invalid API key — please check and try again"
- **Any 2xx:** Key is valid, proceed

On failure, the user stays in the modal and can:
- Re-enter the API key for the same provider
- Go back and pick a different provider

There is no "skip" option — a valid key is required to proceed.

### Config Save Error Handling

If `save_global()` fails during onboarding completion:
- Show an error message in the modal but still allow the user to proceed (the key is already in memory)
- Log a warning so the issue can be diagnosed

## Config Changes

Add `onboarding_completed` field to UI config (semantically fits with `hide_tips`, `theme`, etc.):

```toml
[ui]
onboarding_completed = false  # set to true after first successful key validation
```

This flag is persisted in the global config (`~/.nca/config.toml`) via `save_global()`.

**Note:** This differs from the existing API key save flow which uses `save_workspace_file()`. Onboarding state is user-global, not workspace-specific.

## Files to Modify

| File                                                  | Change                                                                                      |
|-------------------------------------------------------|---------------------------------------------------------------------------------------------|
| `crates/common/src/config.rs`                         | Add `onboarding_completed` to `UiConfig`, `PartialUiConfig`, and `UiConfig::merge()`        |
| `crates/common/src/config.rs`                         | Add `any_api_key_present()` helper to `ProviderConfig`                                       |
| `crates/cli/src/tui/state.rs`                         | Add `onboarding_mode` and `validation_result` fields to `TuiSessionState`                    |
| `crates/cli/src/tui/app.rs`                           | Check onboarding on init, lock modal, disable Escape when in onboarding, send validation cmd |
| `crates/cli/src/repl.rs`                              | Handle `TuiCmd::ValidateApiKey` on the async side, push result to shared state               |
| `crates/core/src/provider/factory.rs` or per-provider | Add `validate_key()` method per provider                                                     |

## Scope Exclusions

- The `/connect` modal UI itself is **not changing** — reused as-is
- The API key modal is **reused as-is**
- **No model selection** during onboarding — uses provider default
- **REPL mode** is not affected — onboarding only applies to TUI mode. The existing "missing API key" error from the provider factory is sufficient for REPL users.
- No welcome message or tips screen after onboarding — user goes straight to chat

## Approach

**TUI-level gate (Approach A):** All logic lives in the TUI layer where modals already exist. No pre-TUI CLI checks, no duplicate flows. The existing `/connect` and API key modals are reused with minimal modifications (locked mode + validation).
