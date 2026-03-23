//! Standalone first-run onboarding TUI.
//!
//! Runs before the session runtime is created, so it has no provider or
//! supervisor. Its only job is to collect and validate an API key, then
//! persist it to the global config.

use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;
use nca_common::config::{NcaConfig, ProviderKind};

use super::app::{setup_terminal, restore_terminal};
use super::connect_modal::{ConnectRow, build_connect_rows, selectable_row_indices};
use super::state::OnboardingValidation;

/// Runs the onboarding TUI. Returns the updated config with the validated key
/// and onboarding_completed = true, or an error if the user quits (Ctrl+C).
pub async fn run_onboarding(mut config: NcaConfig) -> anyhow::Result<NcaConfig> {
    // State for the onboarding flow
    let mut connect_open = true;
    let mut connect_search = String::new();
    let mut connect_index: usize = 0;

    let mut api_key_open = false;
    let mut api_key_input = String::new();
    let mut api_key_provider: Option<ProviderKind> = None;
    let mut validation_status: Option<OnboardingValidation> = None;

    let mut terminal = setup_terminal()?;

    loop {
        // Render
        terminal.draw(|f| {
            let area = f.area();

            // Dark background
            let bg = Block::default().style(Style::default().bg(Color::Black));
            f.render_widget(bg, area);

            // Title
            let title = Paragraph::new("Welcome to nca -- connect a provider to get started")
                .style(Style::default().fg(Color::White))
                .alignment(ratatui::layout::Alignment::Center);
            let title_y = area.height / 4;
            f.render_widget(
                title,
                Rect {
                    x: 0,
                    y: title_y,
                    width: area.width,
                    height: 1,
                },
            );

            if api_key_open {
                render_api_key_modal(f, area, api_key_provider, &api_key_input, &validation_status);
            } else if connect_open {
                render_connect_modal(f, area, &connect_search, connect_index);
            }
        })?;

        // Key is valid — save and exit
        if let Some(OnboardingValidation::Valid) = &validation_status {
            if let Some(provider) = api_key_provider {
                config.set_provider_api_key(provider, &api_key_input);
                config.set_default_provider(provider);
                config.ui.onboarding_completed = true;
                if let Err(e) = config.save_global() {
                    tracing::warn!("onboarding: failed to save global config: {e}");
                }
            }
            restore_terminal();
            return Ok(config);
        }

        // Poll events
        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
                // Global quit
                if key.code == KeyCode::Char('c')
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    restore_terminal();
                    anyhow::bail!("onboarding cancelled by user");
                }

                if api_key_open {
                    match (key.code, key.modifiers) {
                        (KeyCode::Esc, _) => {
                            // Go back to connect
                            api_key_open = false;
                            api_key_input.clear();
                            api_key_provider = None;
                            validation_status = None;
                            connect_open = true;
                        }
                        (KeyCode::Enter, _) => {
                            if let Some(provider) = api_key_provider {
                                let key_str = api_key_input.trim().to_string();
                                if !key_str.is_empty()
                                    && !matches!(
                                        validation_status,
                                        Some(OnboardingValidation::Validating)
                                    )
                                {
                                    // Note: Validating state won't render since
                                    // the await blocks, but we set it for the
                                    // guard check above.
                                    let _ = validation_status
                                        .insert(OnboardingValidation::Validating);
                                    let base_url =
                                        config.provider.base_url_for(provider).to_string();
                                    let result =
                                        nca_core::provider::validate::validate_api_key(
                                            provider, &key_str, &base_url,
                                        )
                                        .await;
                                    match result {
                                        nca_core::provider::validate::ValidationResult::Valid => {
                                            validation_status =
                                                Some(OnboardingValidation::Valid);
                                        }
                                        nca_core::provider::validate::ValidationResult::InvalidKey(
                                            msg,
                                        ) => {
                                            validation_status =
                                                Some(OnboardingValidation::Failed(msg));
                                        }
                                        nca_core::provider::validate::ValidationResult::NetworkError(
                                            msg,
                                        ) => {
                                            validation_status =
                                                Some(OnboardingValidation::Failed(msg));
                                        }
                                    }
                                }
                            }
                        }
                        (KeyCode::Backspace, _) => {
                            api_key_input.pop();
                            validation_status = None;
                        }
                        (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                            api_key_input.push(c);
                            validation_status = None;
                        }
                        _ => {}
                    }
                } else if connect_open {
                    let rows = build_connect_rows(&connect_search);
                    let sel_indices = selectable_row_indices(&rows);
                    let n_sel = sel_indices.len();

                    match (key.code, key.modifiers) {
                        (KeyCode::Up, _) => {
                            if n_sel > 0 {
                                connect_index = connect_index.saturating_sub(1);
                            }
                        }
                        (KeyCode::Down, _) => {
                            if n_sel > 0 {
                                connect_index = (connect_index + 1).min(n_sel - 1);
                            }
                        }
                        (KeyCode::Enter, _) => {
                            if let Some(&row_idx) = sel_indices.get(connect_index)
                                && let ConnectRow::Provider { kind, .. } = &rows[row_idx]
                            {
                                api_key_provider = Some(*kind);
                                api_key_open = true;
                                connect_open = false;
                                api_key_input.clear();
                                validation_status = None;
                            }
                        }
                        (KeyCode::Backspace, _) => {
                            connect_search.pop();
                            connect_index = 0;
                        }
                        (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                            connect_search.push(c);
                            connect_index = 0;
                        }
                        _ => {}
                    }
                }
        }
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

fn render_connect_modal(
    f: &mut Frame,
    area: Rect,
    search: &str,
    selected: usize,
) {
    let modal_rect = centered_rect(50, 14, area);
    f.render_widget(Clear, modal_rect);

    let block = Block::default()
        .title(" Connect a Provider ")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::DarkGray));
    let inner = block.inner(modal_rect);
    f.render_widget(block, modal_rect);

    let rows = build_connect_rows(search);
    let sel_indices = selectable_row_indices(&rows);

    let mut lines = Vec::new();

    // Search bar
    let search_display = if search.is_empty() {
        "Type to filter...".to_string()
    } else {
        search.to_string()
    };
    lines.push(Line::from(vec![
        Span::styled("/ ", Style::default().fg(Color::Yellow)),
        Span::styled(
            search_display,
            Style::default().fg(if search.is_empty() {
                Color::DarkGray
            } else {
                Color::White
            }),
        ),
    ]));
    lines.push(Line::from(""));

    for (i, row) in rows.iter().enumerate() {
        match row {
            ConnectRow::SectionHeader(label) => {
                lines.push(Line::from(Span::styled(
                    format!("  {label}"),
                    Style::default().fg(Color::Yellow),
                )));
            }
            ConnectRow::Provider { title, subtitle, .. } => {
                let is_selected =
                    sel_indices.iter().position(|&si| si == i) == Some(selected);
                let style = if is_selected {
                    Style::default().fg(Color::Black).bg(Color::White)
                } else {
                    Style::default().fg(Color::White)
                };
                let prefix = if is_selected { "> " } else { "  " };
                lines.push(Line::from(Span::styled(
                    format!("{prefix}{title} -- {subtitle}"),
                    style,
                )));
            }
        }
    }

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);
}

fn render_api_key_modal(
    f: &mut Frame,
    area: Rect,
    provider: Option<ProviderKind>,
    input: &str,
    validation: &Option<OnboardingValidation>,
) {
    let modal_rect = centered_rect(50, 10, area);
    f.render_widget(Clear, modal_rect);

    let provider_name = provider.map(|p| p.display_name()).unwrap_or("Provider");
    let block = Block::default()
        .title(format!(" API Key for {provider_name} "))
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::DarkGray));
    let inner = block.inner(modal_rect);
    f.render_widget(block, modal_rect);

    let masked: String = "*".repeat(input.len());
    let mut lines = vec![
        Line::from(Span::styled(
            "Paste your API key:",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(Span::styled(
            if masked.is_empty() { "..." } else { &masked },
            Style::default().fg(if masked.is_empty() {
                Color::DarkGray
            } else {
                Color::Green
            }),
        )),
        Line::from(""),
    ];

    // Validation status
    match validation {
        Some(OnboardingValidation::Validating) => {
            lines.push(Line::from(Span::styled(
                "Validating...",
                Style::default().fg(Color::Yellow),
            )));
        }
        Some(OnboardingValidation::Failed(msg)) => {
            lines.push(Line::from(Span::styled(
                msg.as_str(),
                Style::default().fg(Color::Red),
            )));
        }
        _ => {
            lines.push(Line::from(Span::styled(
                "Press Enter to validate | Esc to go back",
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);
}
