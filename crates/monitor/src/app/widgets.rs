//! Shared egui widgets and layout helpers.

use super::palette;
use super::types::{ChatRole, ChatEntry, SettingsScope};
use eframe::egui;
use nca_common::config::{NcaConfig, PermissionMode, ProviderKind};
use nca_common::orchestration::TodoPriority;
use nca_common::orchestration::TodoStatus;
use nca_common::session::SessionStatus;

pub(crate) fn render_chat_entry(ui: &mut egui::Ui, item: &ChatEntry) {
    let is_user = item.role == ChatRole::User;
    let max_w = ui.available_width() * 0.80;

    if is_user {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
            egui::Frame::none()
                .fill(palette::USER_BUBBLE)
                .rounding(egui::Rounding {
                    nw: 16.0,
                    ne: 4.0,
                    sw: 16.0,
                    se: 16.0,
                })
                .inner_margin(egui::Margin::symmetric(16.0, 12.0))
                .show(ui, |ui| {
                    ui.set_max_width(max_w);
                    ui.colored_label(
                        egui::Color32::from_rgb(180, 210, 255),
                        egui::RichText::new(&item.title).size(9.0).strong(),
                    );
                    ui.add_space(4.0);
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&item.content)
                                .size(13.0)
                                .color(palette::WHITE),
                        )
                        .wrap_mode(egui::TextWrapMode::Wrap),
                    );
                });
        });
    } else {
        let (fill, title_color, is_mono) = match item.role {
            ChatRole::Assistant => (palette::ASSISTANT_BUBBLE, palette::ACCENT, false),
            ChatRole::Tool => (palette::TOOL_BUBBLE, palette::TEXT_DIM, true),
            ChatRole::Error => (palette::ERROR_BUBBLE, palette::ERROR, true),
            ChatRole::User => unreachable!(),
        };

        ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
            egui::Frame::none()
                .fill(fill)
                .rounding(egui::Rounding {
                    nw: 4.0,
                    ne: 16.0,
                    sw: 16.0,
                    se: 16.0,
                })
                .stroke(egui::Stroke::new(1.0, palette::BORDER))
                .inner_margin(egui::Margin::symmetric(16.0, 12.0))
                .show(ui, |ui| {
                    ui.set_max_width(max_w);
                    ui.colored_label(
                        title_color,
                        egui::RichText::new(&item.title).size(9.0).strong(),
                    );
                    ui.add_space(4.0);

                    let mut text = egui::RichText::new(&item.content).color(palette::TEXT);
                    if is_mono {
                        text = text.monospace().size(12.0);
                    } else {
                        text = text.size(13.0);
                    }
                    ui.add(egui::Label::new(text).wrap_mode(egui::TextWrapMode::Wrap));
                });
        });
    }
    ui.add_space(10.0);
}

pub(crate) fn draw_separator(ui: &mut egui::Ui) {
    let rect = ui.available_rect_before_wrap();
    let y = rect.top();
    ui.painter().line_segment(
        [
            egui::pos2(rect.left() + 8.0, y),
            egui::pos2(rect.right() - 8.0, y),
        ],
        egui::Stroke::new(1.0, palette::BORDER),
    );
    ui.add_space(1.0);
}

pub(crate) fn truncate_path(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        s.to_string()
    } else {
        format!("...{}", &s[s.len() - max_chars..])
    }
}

pub(crate) fn format_time(dt: &chrono::DateTime<chrono::Utc>) -> String {
    dt.format("%b %d, %H:%M UTC").to_string()
}

pub(crate) fn session_badge(
    status: &SessionStatus,
) -> (egui::Color32, egui::Color32, &'static str) {
    match status {
        SessionStatus::Running => (palette::ACCENT_BG, palette::ACCENT, "RUNNING"),
        SessionStatus::Completed => (
            egui::Color32::from_rgb(20, 20, 20),
            palette::TEXT_DIM,
            "COMPLETED",
        ),
        SessionStatus::Error => (palette::ERROR_BUBBLE, palette::ERROR, "ERROR"),
        SessionStatus::Cancelled => (
            egui::Color32::from_rgb(20, 20, 20),
            palette::WARNING,
            "CANCELLED",
        ),
    }
}

pub(crate) fn scope_tab(
    ui: &mut egui::Ui,
    selected: &mut SettingsScope,
    value: SettingsScope,
    label: &str,
) {
    let is_active = *selected == value;
    let (text_color, underline) = if is_active {
        (palette::ACCENT, true)
    } else {
        (palette::TEXT_DIM, false)
    };

    let resp = ui.add(
        egui::Label::new(
            egui::RichText::new(label)
                .size(13.0)
                .strong()
                .color(text_color),
        )
        .sense(egui::Sense::click()),
    );
    if underline {
        let rect = resp.rect;
        ui.painter().line_segment(
            [
                egui::pos2(rect.left(), rect.bottom() + 2.0),
                egui::pos2(rect.right(), rect.bottom() + 2.0),
            ],
            egui::Stroke::new(2.0, palette::ACCENT),
        );
    }
    if resp.clicked() {
        *selected = value;
    }
    ui.add_space(16.0);
}

pub(crate) fn mode_pill(ui: &mut egui::Ui, active: bool, label: &str) -> egui::Response {
    let text_color = if active { palette::WHITE } else { palette::TEXT_DIM };
    ui.add(
        egui::Button::new(egui::RichText::new(label).size(11.5).color(text_color))
            .fill(if active {
                palette::ACCENT_BG
            } else {
                palette::CARD
            })
            .stroke(egui::Stroke::new(
                1.0,
                if active {
                    palette::ACCENT
                } else {
                    palette::BORDER
                },
            ))
            .rounding(6.0)
            .min_size(egui::vec2(0.0, 26.0)),
    )
}

pub(crate) fn draw_nav_link(ui: &mut egui::Ui, is_active: bool, label: &str) -> egui::Response {
    let text_color = if is_active {
        palette::WHITE
    } else {
        palette::TEXT
    };
    let bg = if is_active {
        palette::ACCENT_BG
    } else {
        egui::Color32::TRANSPARENT
    };
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 34.0), egui::Sense::click());
    let inner = rect.shrink2(egui::vec2(10.0, 1.0));

    ui.painter().rect_filled(inner, 6.0, bg);
    if is_active {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(inner.left_top(), egui::vec2(3.0, inner.height())),
            2.0,
            palette::ACCENT,
        );
    }

    ui.painter().text(
        egui::pos2(inner.left() + 22.0, inner.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(13.5),
        text_color,
    );

    response
}

pub(crate) fn section_label(ui: &mut egui::Ui, label: &str) {
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.colored_label(
            palette::TEXT_DIM,
            egui::RichText::new(label).size(10.5).strong(),
        );
    });
    ui.add_space(4.0);
}

pub(crate) fn draw_entity_tile(
    ui: &mut egui::Ui,
    selected: bool,
    title: &str,
    subtitle: &str,
) -> egui::Response {
    let (border_color, bg_color) = if selected {
        (palette::ACCENT, palette::ACCENT_BG)
    } else {
        (palette::BORDER, palette::CARD)
    };
    let outer_rect = ui.available_rect_before_wrap();
    let desired = egui::vec2((outer_rect.width() - 24.0).max(40.0), 44.0);
    let resp = ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), 48.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.add_space(12.0);
            let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click());
            ui.painter()
                .rect(rect, 6.0, bg_color, egui::Stroke::new(1.0, border_color));
            let title_color = if selected {
                palette::WHITE
            } else {
                palette::TEXT
            };
            ui.painter().text(
                rect.left_top() + egui::vec2(10.0, 7.0),
                egui::Align2::LEFT_TOP,
                truncate_path(title, 26),
                egui::FontId::proportional(12.5),
                title_color,
            );
            if !subtitle.is_empty() {
                ui.painter().text(
                    rect.left_top() + egui::vec2(10.0, 25.0),
                    egui::Align2::LEFT_TOP,
                    truncate_path(subtitle, 30),
                    egui::FontId::proportional(10.5),
                    palette::TEXT_DIM,
                );
            }
            resp
        },
    );
    ui.add_space(2.0);
    resp.inner
}

pub(crate) fn stat_card(ui: &mut egui::Ui, label: &str, value: &str) {
    egui::Frame::none()
        .fill(palette::CARD)
        .rounding(10.0)
        .stroke(egui::Stroke::new(1.0, palette::BORDER))
        .inner_margin(egui::Margin::symmetric(16.0, 12.0))
        .show(ui, |ui| {
            ui.set_min_width(140.0);
            ui.colored_label(
                palette::TEXT_DIM,
                egui::RichText::new(label).size(11.0).strong(),
            );
            ui.add_space(4.0);
            ui.colored_label(
                palette::WHITE,
                egui::RichText::new(value).size(22.0).strong(),
            );
        });
}

pub(crate) fn panel_card(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::none()
        .fill(palette::CARD)
        .rounding(12.0)
        .stroke(egui::Stroke::new(1.0, palette::BORDER))
        .inner_margin(egui::Margin::symmetric(16.0, 14.0))
        .show(ui, |ui| {
            ui.colored_label(
                palette::WHITE,
                egui::RichText::new(title).size(14.0).strong(),
            );
            ui.add_space(10.0);
            add_contents(ui);
        });
    ui.add_space(10.0);
}

pub(crate) fn todo_priority_combo(ui: &mut egui::Ui, priority: &mut TodoPriority) {
    egui::ComboBox::from_id_salt("todo_priority")
        .selected_text(match priority {
            TodoPriority::Low => "Priority: Low",
            TodoPriority::Medium => "Priority: Medium",
            TodoPriority::High => "Priority: High",
            TodoPriority::Critical => "Priority: Critical",
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(priority, TodoPriority::Low, "Low");
            ui.selectable_value(priority, TodoPriority::Medium, "Medium");
            ui.selectable_value(priority, TodoPriority::High, "High");
            ui.selectable_value(priority, TodoPriority::Critical, "Critical");
        });
}

pub(crate) fn todo_status_combo(ui: &mut egui::Ui, status: &mut TodoStatus) {
    egui::ComboBox::from_id_salt("todo_status")
        .selected_text(todo_status_label(*status))
        .show_ui(ui, |ui| {
            for candidate in [
                TodoStatus::Backlog,
                TodoStatus::Ready,
                TodoStatus::InProgress,
                TodoStatus::InReview,
                TodoStatus::Blocked,
                TodoStatus::Done,
                TodoStatus::Cancelled,
            ] {
                ui.selectable_value(status, candidate, todo_status_label(candidate));
            }
        });
}

pub(crate) fn todo_status_label(status: TodoStatus) -> &'static str {
    match status {
        TodoStatus::Backlog => "Backlog",
        TodoStatus::Ready => "Ready",
        TodoStatus::InProgress => "In Progress",
        TodoStatus::InReview => "In Review",
        TodoStatus::Blocked => "Blocked",
        TodoStatus::Done => "Done",
        TodoStatus::Cancelled => "Cancelled",
    }
}

pub(crate) fn clean_optional_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(crate) fn show_config_form(ui: &mut egui::Ui, config: &mut NcaConfig, is_project: bool) {
    egui::Frame::none()
        .fill(palette::CARD)
        .rounding(10.0)
        .stroke(egui::Stroke::new(1.0, palette::BORDER))
        .inner_margin(egui::Margin::symmetric(20.0, 16.0))
        .show(ui, |ui| {
            config_row(ui, "PROVIDER", |ui| {
                egui::ComboBox::from_id_salt(("provider", is_project))
                    .selected_text(provider_label(&config.provider.default))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut config.provider.default,
                            ProviderKind::MiniMax,
                            "MiniMax",
                        );
                        ui.add_enabled_ui(false, |ui| {
                            let _ = ui.selectable_label(false, "OpenRouter (coming soon)");
                            let _ = ui.selectable_label(false, "Anthropic (coming soon)");
                            let _ = ui.selectable_label(false, "OpenAI (coming soon)");
                        });
                    });
            });
            ui.add_space(8.0);
            config_row(ui, "API KEY", |ui| {
                ui.add(
                    egui::TextEdit::singleline(
                        config
                            .provider
                            .minimax
                            .api_key
                            .get_or_insert_with(String::new),
                    )
                    .password(true)
                    .desired_width(300.0),
                );
            });
            ui.add_space(8.0);
            config_row(ui, "API KEY ENV VAR", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut config.provider.minimax.api_key_env)
                        .desired_width(300.0),
                );
            });
            ui.add_space(8.0);
            config_row(ui, "BASE URL", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut config.provider.minimax.base_url)
                        .desired_width(300.0),
                );
            });
            ui.add_space(8.0);
            config_row(ui, "DEFAULT MODEL", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut config.model.default_model)
                        .desired_width(300.0),
                );
            });
            config.provider.minimax.model = config.model.default_model.clone();
            ui.add_space(8.0);
            config_row(ui, "PERMISSION MODE", |ui| {
                permission_mode_combo(ui, &mut config.permissions.mode);
            });
        });
    ui.add_space(4.0);
    ui.colored_label(
        palette::TEXT_DIM,
        egui::RichText::new(
            "Only MiniMax is implemented. Other providers stay disabled until their runtime support lands.",
        )
        .size(10.0),
    );
}

fn config_row(ui: &mut egui::Ui, label: &str, add_widget: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(130.0, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.colored_label(
                    palette::TEXT_DIM,
                    egui::RichText::new(label).size(9.0).strong(),
                );
            },
        );
        add_widget(ui);
    });
}

pub(crate) fn permission_mode_combo(ui: &mut egui::Ui, mode: &mut PermissionMode) {
    egui::ComboBox::from_id_salt("perm_mode")
        .selected_text(permission_label(*mode))
        .show_ui(ui, |ui| {
            for candidate in [
                PermissionMode::AcceptEdits,
                PermissionMode::Default,
                PermissionMode::Plan,
                PermissionMode::DontAsk,
                PermissionMode::BypassPermissions,
            ] {
                ui.selectable_value(mode, candidate, permission_label(candidate));
            }
        });
}

fn permission_label(mode: PermissionMode) -> &'static str {
    match mode {
        PermissionMode::Default => "Default",
        PermissionMode::Plan => "Plan (read-only)",
        PermissionMode::AcceptEdits => "Accept edits",
        PermissionMode::DontAsk => "Don't ask",
        PermissionMode::BypassPermissions => "Bypass permissions",
    }
}

fn provider_label(provider: &ProviderKind) -> &'static str {
    match provider {
        ProviderKind::MiniMax => "MiniMax",
        ProviderKind::OpenRouter => "OpenRouter",
        ProviderKind::Anthropic => "Anthropic",
        ProviderKind::OpenAi => "OpenAI",
    }
}
