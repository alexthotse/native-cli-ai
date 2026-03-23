# Onboarding Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Block chat until the user validates an API key on first run, using the existing `/connect` modal in locked mode.

**Architecture:** Add `onboarding_completed` to `UiConfig`, add `any_api_key_present()` to `ProviderConfig`, add `validate_api_key()` as a standalone async function, wire a new `TuiCmd::ValidateApiKey` variant through repl → TUI state, and guard the connect modal's Escape handler behind an `onboarding_mode` flag.

**Tech Stack:** Rust, ratatui, crossterm, reqwest, tokio, serde, toml

**Spec:** `docs/superpowers/specs/2026-03-23-onboarding-gate-design.md`

---

### Task 1: Add `onboarding_completed` to UiConfig

**Files:**
- Modify: `crates/common/src/config.rs` (UiConfig ~line 289, PartialUiConfig ~line 1277, UiConfig::merge ~line 320)

- [ ] **Step 1: Write the failing test**

In `crates/common/src/config.rs`, add at the bottom of the existing `#[cfg(test)] mod tests` block:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p nca-common onboarding_completed`
Expected: FAIL — `onboarding_completed` field does not exist

- [ ] **Step 3: Add `onboarding_completed` field to `UiConfig`**

In `UiConfig` struct (~line 289), add after `scroll_speed`:

```rust
    /// Whether the user has completed the first-run onboarding flow.
    #[serde(default)]
    pub onboarding_completed: bool,
```

In `PartialUiConfig` (~line 1277), add:

```rust
    onboarding_completed: Option<bool>,
```

In `UiConfig::merge()` (~line 320), add before the closing brace:

```rust
        if let Some(onboarding_completed) = partial.onboarding_completed {
            self.onboarding_completed = onboarding_completed;
        }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p nca-common onboarding_completed`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/common/src/config.rs
git commit -m "feat: add onboarding_completed field to UiConfig"
```

---

### Task 2: Add `any_api_key_present()` to ProviderConfig

**Files:**
- Modify: `crates/common/src/config.rs` (ProviderConfig impl, near `api_key_present_for` ~line 572)

- [ ] **Step 1: Write the failing test**

In `crates/common/src/config.rs` tests block:

```rust
#[test]
fn any_api_key_present_returns_false_when_no_keys() {
    let config = NcaConfig::default();
    assert!(!config.provider.any_api_key_present());
}

#[test]
fn any_api_key_present_returns_true_when_one_key_set() {
    let mut config = NcaConfig::default();
    config.provider.openai.api_key = Some("sk-test".into());
    assert!(config.provider.any_api_key_present());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p nca-common any_api_key_present`
Expected: FAIL — method does not exist

- [ ] **Step 3: Implement `any_api_key_present()`**

In the `impl ProviderConfig` block, right after `api_key_present_for()` (~line 579):

```rust
    /// Returns `true` if at least one provider has an API key configured
    /// (either in config or via environment variable).
    pub fn any_api_key_present(&self) -> bool {
        ProviderKind::ALL.iter().any(|p| self.api_key_present_for(*p))
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p nca-common any_api_key_present`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/common/src/config.rs
git commit -m "feat: add any_api_key_present() helper to ProviderConfig"
```

---

### Task 3: Add `needs_onboarding()` helper to NcaConfig

**Files:**
- Modify: `crates/common/src/config.rs` (NcaConfig impl)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn needs_onboarding_true_when_no_flag_and_no_keys() {
    let config = NcaConfig::default();
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
    let mut config = NcaConfig::default();
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p nca-common needs_onboarding`
Expected: FAIL — method does not exist

- [ ] **Step 3: Implement `needs_onboarding()`**

In `impl NcaConfig`, add after `sync_default_model_from_provider()` (~line 284):

```rust
    /// Returns `true` if the first-run onboarding gate should be shown.
    /// Triggers when: onboarding not completed OR all API keys have been removed.
    pub fn needs_onboarding(&self) -> bool {
        !self.ui.onboarding_completed || !self.provider.any_api_key_present()
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p nca-common needs_onboarding`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/common/src/config.rs
git commit -m "feat: add needs_onboarding() detection logic to NcaConfig"
```

---

### Task 4: Add `validate_api_key()` function

**Files:**
- Create: `crates/core/src/provider/validate.rs`
- Modify: `crates/core/src/provider.rs` (add `pub mod validate;`)

- [ ] **Step 1: Write the failing test**

Create `crates/core/src/provider/validate.rs` with tests only first:

```rust
//! Lightweight API key validation per provider.

use nca_common::config::ProviderKind;

/// Result of an API key validation attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationResult {
    Valid,
    InvalidKey(String),
    NetworkError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_result_variants_exist() {
        let valid = ValidationResult::Valid;
        let invalid = ValidationResult::InvalidKey("bad key".into());
        let net_err = ValidationResult::NetworkError("timeout".into());
        assert_eq!(valid, ValidationResult::Valid);
        assert!(matches!(invalid, ValidationResult::InvalidKey(_)));
        assert!(matches!(net_err, ValidationResult::NetworkError(_)));
    }
}
```

- [ ] **Step 2: Register the module**

In `crates/core/src/provider.rs`, add after `pub mod openrouter;` (~line 8):

```rust
pub mod validate;
```

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test -p nca-core validation_result`
Expected: PASS

- [ ] **Step 4: Implement the `validate_api_key()` async function**

Add to `crates/core/src/provider/validate.rs`, above the `#[cfg(test)]`:

```rust
use std::time::Duration;

use reqwest::StatusCode;

/// Validate an API key by making a lightweight request to the provider.
///
/// - OpenAI / OpenRouter: `GET /v1/models`
/// - Anthropic / MiniMax: `POST /v1/messages` with minimal body
pub async fn validate_api_key(
    provider: ProviderKind,
    api_key: &str,
    base_url: &str,
) -> ValidationResult {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => return ValidationResult::NetworkError(format!("failed to build client: {e}")),
    };

    let result = match provider {
        ProviderKind::OpenAi | ProviderKind::OpenRouter => {
            let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
            client
                .get(&url)
                .header("Authorization", format!("Bearer {api_key}"))
                .send()
                .await
        }
        ProviderKind::Anthropic => {
            let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));
            client
                .post(&url)
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .body(r#"{"model":"claude-3-haiku-20240307","max_tokens":1,"messages":[{"role":"user","content":"hi"}]}"#)
                .send()
                .await
        }
        ProviderKind::MiniMax => {
            let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));
            client
                .post(&url)
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .body(r#"{"model":"MiniMax-M1","max_tokens":1,"messages":[{"role":"user","content":"hi"}]}"#)
                .send()
                .await
        }
    };

    match result {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                ValidationResult::Valid
            } else if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                ValidationResult::InvalidKey(
                    "Invalid API key — please check and try again".into(),
                )
            } else {
                // Some providers return 400 for minimal requests but the key is valid.
                // A 400 with auth headers accepted means the key works.
                if status == StatusCode::BAD_REQUEST {
                    ValidationResult::Valid
                } else {
                    ValidationResult::NetworkError(format!("unexpected status: {status}"))
                }
            }
        }
        Err(e) => {
            if e.is_timeout() {
                ValidationResult::NetworkError(
                    "Connection timed out — check your network and try again".into(),
                )
            } else {
                ValidationResult::NetworkError(format!(
                    "Connection failed — check your network and try again ({e})"
                ))
            }
        }
    }
}
```

- [ ] **Step 5: Run all tests**

Run: `cargo test -p nca-core validation_result`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/provider/validate.rs crates/core/src/provider.rs
git commit -m "feat: add validate_api_key() for per-provider key validation"
```

---

### Task 5: Add onboarding state fields to TuiSessionState

**Files:**
- Modify: `crates/cli/src/tui/state.rs`

- [ ] **Step 1: Add `onboarding_mode` and `validation_status` fields**

In `TuiSessionState` struct (~line 53), add after `session_picker_scroll` (~line 154):

```rust
    /// When true, the onboarding gate is active — connect modal is locked open.
    pub onboarding_mode: bool,
    /// Result of the most recent API key validation attempt (None = no attempt yet).
    pub validation_status: Option<OnboardingValidation>,
```

Add the enum above the struct:

```rust
/// Status of an API key validation during onboarding.
#[derive(Debug, Clone)]
pub enum OnboardingValidation {
    Validating,
    Valid,
    Failed(String),
}
```

- [ ] **Step 2: Initialize the new fields in `TuiSessionState::new()`**

In the `Self { ... }` block (~line 244), add before the closing brace:

```rust
            onboarding_mode: false,
            validation_status: None,
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p nca-cli`
Expected: success (possibly with warnings about unused fields, which is fine)

- [ ] **Step 4: Commit**

```bash
git add crates/cli/src/tui/state.rs
git commit -m "feat: add onboarding_mode and validation_status to TuiSessionState"
```

---

### Task 6: Add `TuiCmd::ValidateApiKey` variant and wire async handler

**Files:**
- Modify: `crates/cli/src/tui/app.rs` (TuiCmd enum ~line 46)
- Modify: `crates/cli/src/repl.rs` (TuiCmd handler, near ~line 1708)

- [ ] **Step 1: Add the TuiCmd variants**

In the `TuiCmd` enum (~line 46 in `app.rs`), add after `CycleModel(bool)`:

```rust
    /// Validate an API key for onboarding (provider, api_key).
    /// The repl handler looks up base_url from config.
    ValidateApiKey(ProviderKind, String),
    /// Mark onboarding as complete and persist the flag.
    CompleteOnboarding,
```

Note: `ValidateApiKey` takes only `(ProviderKind, String)` — the repl handler looks up `base_url` from `self.runtime.config().provider.base_url_for(provider)`. This avoids `app.rs` needing config access.

- [ ] **Step 2: Handle `ValidateApiKey` in repl.rs**

In `repl.rs`, find the `TuiCmd` match block (the large `match cmd { ... }` that handles `TuiCmd::PromptApiKey` at ~line 1712). Add the new arms near it:

```rust
TuiCmd::ValidateApiKey(provider, api_key) => {
    // Set validating state
    if let Ok(mut g) = tui_state.lock() {
        g.validation_status = Some(crate::tui::state::OnboardingValidation::Validating);
    }
    // Look up base_url from config
    let base_url = self.runtime.config().provider.base_url_for(provider).to_string();
    // Run async validation
    let result = nca_core::provider::validate::validate_api_key(
        provider,
        &api_key,
        &base_url,
    ).await;
    if let Ok(mut g) = tui_state.lock() {
        match result {
            nca_core::provider::validate::ValidationResult::Valid => {
                // Save key and complete onboarding
                g.validation_status = Some(crate::tui::state::OnboardingValidation::Valid);
                g.close_api_key_modal();
                g.close_connect_modal();
                g.onboarding_mode = false;
            }
            nca_core::provider::validate::ValidationResult::InvalidKey(msg) => {
                g.validation_status = Some(crate::tui::state::OnboardingValidation::Failed(msg));
            }
            nca_core::provider::validate::ValidationResult::NetworkError(msg) => {
                g.validation_status = Some(crate::tui::state::OnboardingValidation::Failed(msg));
            }
        }
    }
    // If validation succeeded, save key + complete onboarding
    if matches!(result, nca_core::provider::validate::ValidationResult::Valid) {
        self.save_provider_api_key(provider, &api_key, ReplOutput::Tui(&tui_state))
            .await?;
        self.apply_provider_in_session(provider, ReplOutput::Tui(&tui_state))
            .await?;
        // Send CompleteOnboarding via self-handling (inline)
        let mut cfg = self.runtime.config().clone();
        cfg.ui.onboarding_completed = true;
        if let Err(e) = cfg.save_global() {
            tracing::warn!("onboarding flag save failed: {e}");
            // Proceed anyway — key is in memory
        }
        if let Err(e) = self.runtime.apply_nca_config(cfg) {
            tracing::warn!("onboarding config apply failed: {e}");
        }
    }
}
TuiCmd::CompleteOnboarding => {
    let mut cfg = self.runtime.config().clone();
    cfg.ui.onboarding_completed = true;
    if let Err(e) = cfg.save_global() {
        tracing::warn!("onboarding flag save failed: {e}");
    }
    if let Err(e) = self.runtime.apply_nca_config(cfg) {
        tracing::warn!("onboarding config apply failed: {e}");
    }
}
```

Note: The `ValidateApiKey` handler does everything inline — validation, key save via existing `save_provider_api_key()`, provider connection via `apply_provider_in_session()`, and onboarding completion. This reuses the existing API key save flow and avoids fabricating new command protocols.

- [ ] **Step 3: Add import for validate module**

At the top of `repl.rs`, ensure `nca_core::provider::validate` is accessible. Since `repl.rs` already depends on `nca_core`, no `Cargo.toml` change is needed.

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p nca-cli`
Expected: success

- [ ] **Step 5: Commit**

```bash
git add crates/cli/src/tui/app.rs crates/cli/src/repl.rs
git commit -m "feat: add ValidateApiKey and CompleteOnboarding TuiCmd handlers"
```

---

### Task 7: Wire onboarding gate into TUI initialization and modal logic

**Files:**
- Modify: `crates/cli/src/repl.rs` (onboarding check after state creation, ~line 1553)
- Modify: `crates/cli/src/tui/app.rs` (Escape handlers, Enter handler, validation status rendering)

- [ ] **Step 1: Trigger onboarding mode on TUI init**

In `crates/cli/src/repl.rs`, in `run_with_tui()`, right after the `TuiSessionState` is created (~line 1553) and before `replay_event_log_into_state` (~line 1556), add:

```rust
        // Check if first-run onboarding is needed
        if self.runtime.config().needs_onboarding() {
            if let Ok(mut g) = tui_state.lock() {
                g.onboarding_mode = true;
                g.open_connect_modal();
            }
        }
```

- [ ] **Step 2: Guard Escape key in connect modal**

In `app.rs`, find the Escape handler for the connect modal (~line 2706):

```rust
(KeyCode::Esc, _) => {
    g.close_connect_modal();
}
```

Change to:

```rust
(KeyCode::Esc, _) => {
    if !g.onboarding_mode {
        g.close_connect_modal();
    }
}
```

- [ ] **Step 3: Guard Escape key in API key modal during onboarding**

Find the Escape handler for the API key modal (~line 2752). Change:

```rust
(KeyCode::Esc, _) => {
    g.close_api_key_modal();
}
```

To:

```rust
(KeyCode::Esc, _) => {
    g.close_api_key_modal();
    if g.onboarding_mode {
        // Go back to connect modal instead of closing entirely
        g.open_connect_modal();
    }
}
```

- [ ] **Step 4: Intercept API key Enter during onboarding**

In `app.rs`, find the API key modal Enter handler (~line 2755):

```rust
(KeyCode::Enter, _) => {
    drop(g);
    let _ = cmd_tx.send(TuiCmd::Submit(String::new()));
}
```

Change to:

```rust
(KeyCode::Enter, _) => {
    if g.onboarding_mode {
        if let Some(provider) = g.api_key_target_provider {
            let key = g.api_key_input.clone();
            if key.trim().is_empty() {
                // Don't submit empty keys during onboarding
            } else {
                g.validation_status = Some(crate::tui::state::OnboardingValidation::Validating);
                drop(g);
                let _ = cmd_tx.send(TuiCmd::ValidateApiKey(provider, key));
            }
        }
    } else {
        drop(g);
        let _ = cmd_tx.send(TuiCmd::Submit(String::new()));
    }
}
```

This intercepts the Enter key during onboarding and sends `ValidateApiKey` instead of the normal `Submit` flow. The repl handler (Task 6) does the validation, saves the key, and completes onboarding all in one async flow.

- [ ] **Step 5: Keep connect modal open when entering API key modal during onboarding**

In `app.rs`, find the connect modal Enter handler (~line 2721):

```rust
(KeyCode::Enter, _) => {
    if let Some(p) = provider_at_selection(&rows, g.connect_menu_index)
    {
        g.close_connect_modal();
        drop(g);
        let _ = cmd_tx.send(TuiCmd::PromptApiKey(p, true));
    }
}
```

During onboarding, we still close the connect modal and open the API key modal (via `PromptApiKey`), but on Escape from API key modal (Step 3), we re-open the connect modal. No change needed here — the existing flow works because Step 3 handles the back-navigation.

- [ ] **Step 6: Render validation status in the API key modal**

Find the API key modal rendering code in `app.rs`. Add validation status display below the masked input field. When `g.onboarding_mode` and `g.validation_status` is set:

- `Validating` → render `Paragraph::new("Validating...").style(Style::default().fg(Color::Yellow))` below the input
- `Failed(msg)` → render `Paragraph::new(msg.as_str()).style(Style::default().fg(Color::Red))` below the input
- `Valid` → this state is transient (the repl handler clears `onboarding_mode` and closes modals), so no rendering needed

Also clear `validation_status` when the user types a new character (in the `KeyCode::Char` handler for the API key modal), so stale error messages disappear when they start re-entering a key:

```rust
(KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
    g.api_key_input.push(c);
    if g.onboarding_mode {
        g.validation_status = None; // Clear stale error on new input
    }
}
```

- [ ] **Step 7: Verify it compiles and the TUI starts**

Run: `cargo run -p nca-cli` (with no API keys configured — e.g., set `NCA_HOME=/tmp/nca-test`)
Expected: TUI launches with the connect modal open, Escape does nothing

- [ ] **Step 8: Commit**

```bash
git add crates/cli/src/tui/app.rs crates/cli/src/repl.rs
git commit -m "feat: wire onboarding gate into TUI init and modal logic"
```

---

### Task 8: Integration test — onboarding detection

**Files:**
- Modify: `crates/common/src/config.rs` (tests section)

- [ ] **Step 1: Write integration tests for the full onboarding detection flow**

```rust
#[test]
fn onboarding_roundtrip_through_toml() {
    // Simulate: user saves onboarding_completed = true to config
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
    let mut config = NcaConfig::default();
    config.merge(partial);
    // No keys present — safety net triggers
    assert!(config.needs_onboarding());
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p nca-common onboarding`
Expected: PASS (all onboarding tests)

- [ ] **Step 3: Commit**

```bash
git add crates/common/src/config.rs
git commit -m "test: add integration tests for onboarding detection roundtrip"
```

---

### Task 9: Add additional validation tests

**Files:**
- Modify: `crates/core/src/provider/validate.rs` (tests section — extend existing tests from Task 4)

- [ ] **Step 1: Add message-preservation tests**

Add to the existing `#[cfg(test)] mod tests` in `validate.rs`:

```rust
    #[test]
    fn invalid_key_message_preserved() {
        let msg = "Invalid API key — please check and try again";
        let result = ValidationResult::InvalidKey(msg.into());
        match result {
            ValidationResult::InvalidKey(m) => assert_eq!(m, msg),
            _ => panic!("expected InvalidKey"),
        }
    }

    #[test]
    fn network_error_message_preserved() {
        let msg = "Connection timed out — check your network and try again";
        let result = ValidationResult::NetworkError(msg.into());
        match result {
            ValidationResult::NetworkError(m) => assert_eq!(m, msg),
            _ => panic!("expected NetworkError"),
        }
    }
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p nca-core validate`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/provider/validate.rs
git commit -m "test: add message-preservation tests for ValidationResult"
```

---

### Task 10: Final verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test --workspace`
Expected: All tests PASS

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace -- -D warnings`
Expected: No warnings

- [ ] **Step 3: Manual smoke test**

1. Remove any existing API keys from config: `rm -f ~/.nca/config.toml`
2. Run `cargo run -p nca-cli`
3. Verify: Connect modal opens, Escape does nothing
4. Pick a provider, enter a valid API key
5. Verify: Key is validated, modal closes, chat is available
6. Exit and re-run `cargo run -p nca-cli`
7. Verify: No onboarding gate — goes straight to chat

- [ ] **Step 4: Commit any final fixes**

```bash
git add -A
git commit -m "feat: complete onboarding gate implementation"
```
