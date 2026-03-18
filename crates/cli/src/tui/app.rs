//! Beautiful TUI application for nca
//! 
//! A Claude Code-inspired terminal UI with scrollable messages,
//! syntax highlighting, and a clean status bar.

use crossterm::{
    event::{Event as CrosstermEvent, KeyCode, KeyModifiers, poll},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, Clear, ClearType},
    cursor::{Hide, Show},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};
use std::io::stdout;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Message types for display
#[derive(Debug, Clone)]
pub enum MessageType {
    User,
    Assistant,
    System,
    Tool,
    ToolResult,
}

/// A single chat message
#[derive(Debug, Clone)]
pub struct Message {
    pub msg_type: MessageType,
    pub content: String,
    pub timestamp: Instant,
    pub model: Option<String>,
    pub tool_name: Option<String>,
}

/// Application state
pub struct App {
    pub messages: Vec<Message>,
    pub status: AppStatus,
    pub input_buffer: String,
    pub cursor_position: usize,
    pub scroll_offset: usize,
    pub is_running: bool,
    pub session_id: String,
    pub model: String,
    pub show_welcome: bool,
}

#[derive(Debug, Clone)]
pub struct AppStatus {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    pub permission_mode: String,
    pub agent_profile: String,
    pub elapsed_secs: u64,
}

impl Default for App {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            status: AppStatus {
                input_tokens: 0,
                output_tokens: 0,
                cost_usd: 0.0,
                permission_mode: "default".into(),
                agent_profile: "@build".into(),
                elapsed_secs: 0,
            },
            input_buffer: String::new(),
            cursor_position: 0,
            scroll_offset: 0,
            is_running: true,
            session_id: "".into(),
            model: "MiniMax-M2.5".into(),
            show_welcome: true,
        }
    }
}

impl App {
    pub fn new(session_id: String, model: String) -> Self {
        Self {
            session_id,
            model,
            ..Default::default()
        }
    }

    pub fn add_message(&mut self, msg: Message) {
        self.messages.push(msg);
        self.show_welcome = false;
    }

    pub fn update_status(&mut self, status: AppStatus) {
        self.status = status;
    }
}

// Claude Code-inspired dark theme
mod theme {
    use ratatui::style::Color;

    pub const BG: Color = Color::Rgb(30, 30, 36);
    pub const SURFACE: Color = Color::Rgb(40, 40, 48);
    pub const BORDER: Color = Color::Rgb(60, 60, 72);
    
    pub const USER_BG: Color = Color::Rgb(0, 145, 191);
    pub const ASSISTANT_BG: Color = Color::Rgb(137, 87, 220);
    pub const TOOL_BG: Color = Color::Rgb(58, 170, 214);
    
    pub const TEXT: Color = Color::Rgb(220, 220, 230);
    pub const TEXT_DIM: Color = Color::Rgb(150, 150, 160);
    pub const TEXT_BRIGHT: Color = Color::Rgb(255, 255, 255);
    
    pub const SUCCESS: Color = Color::Rgb(63, 185, 80);
    pub const ERROR: Color = Color::Rgb(248, 81, 73);
    pub const WARNING: Color = Color::Rgb(210, 153, 34);
}

/// Setup terminal for TUI
pub fn setup_terminal() -> anyhow::Result<Terminal<CrosstermBackend<std::io::Stdout>>> {
    execute!(stdout(), EnterAlternateScreen)?;
    execute!(stdout(), Hide)?;
    execute!(stdout(), Clear(ClearType::All))?;
    
    let backend = CrosstermBackend::new(stdout());
    let terminal = Terminal::new(backend)?;
    
    Ok(terminal)
}

/// Restore terminal to normal mode
pub fn restore_terminal() {
    let _ = execute!(stdout(), Show);
    let _ = execute!(stdout(), LeaveAlternateScreen);
}

/// Main render function
pub fn render(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>, app: &App) -> anyhow::Result<()> {
    terminal.draw(|frame| {
        let area = frame.area();
        
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(3), // Status bar
                Constraint::Length(3), // Input area
            ])
            .split(area);

        render_messages(frame, chunks[0], app);
        render_status_bar(frame, chunks[1], app);
        render_input(frame, chunks[2], app);
    })?;
    Ok(())
}

fn render_messages(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let border_style = Style::default().fg(theme::BORDER);
    
    if app.show_welcome || app.messages.is_empty() {
        let welcome = Paragraph::new(
            Text::from(vec![
                Line::from(Span::styled("Welcome to ", Style::default().fg(theme::TEXT))),
                Line::from(Span::styled("nca", Style::default().fg(theme::ASSISTANT_BG).bold())),
                Line::from(Span::raw("")),
                Line::from(Span::styled("Type a message to start chatting...", Style::default().fg(theme::TEXT_DIM))),
                Line::from(Span::raw("")),
                Line::from(vec![
                    Span::styled("Tab", Style::default().fg(theme::USER_BG).bold()),
                    Span::styled(" switch agent ", Style::default().fg(theme::TEXT_DIM)),
                    Span::styled("!cmd", Style::default().fg(theme::SUCCESS)),
                    Span::styled(" bash ", Style::default().fg(theme::TEXT_DIM)),
                    Span::styled("@file", Style::default().fg(theme::WARNING)),
                    Span::styled(" search ", Style::default().fg(theme::TEXT_DIM)),
                    Span::styled("/help", Style::default().fg(theme::ASSISTANT_BG)),
                ]),
            ])
        )
        .block(Block::default().borders(Borders::ALL).border_style(border_style))
        .centered();
        frame.render_widget(welcome, area);
        return;
    }

    let total = app.messages.len();
    let start = app.scroll_offset.min(total.saturating_sub(1));
    let visible = area.height as usize;
    let end = (start + visible).min(total);
    
    if start >= end {
        return;
    }

    let visible_messages = &app.messages[start..end];
    let mut lines: Vec<Line> = Vec::new();
    
    for msg in visible_messages {
        lines.extend(render_message_lines(msg));
        lines.push(Line::default());
    }

    let content = Text::from(lines);
    let list = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).border_style(border_style))
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(theme::TEXT));

    frame.render_widget(list, area);
}

fn render_message_lines(msg: &Message) -> Vec<Line> {
    let mut lines = Vec::new();
    
    let (header_bg, header_text) = match msg.msg_type {
        MessageType::User => (theme::USER_BG, " YOU "),
        MessageType::Assistant => (theme::ASSISTANT_BG, " nca "),
        MessageType::System => (theme::WARNING, " SYSTEM "),
        MessageType::Tool => {
            let name = msg.tool_name.as_deref().unwrap_or("TOOL");
            return vec![
                Line::from(Span::styled(
                    format!(" {} ", name.to_uppercase()),
                    Style::default().bg(theme::TOOL_BG).fg(theme::TEXT_BRIGHT).bold(),
                )),
                Line::from(Span::raw("")),
                Line::from(Span::styled(&msg.content, Style::default().fg(theme::TEXT))),
            ];
        }
        MessageType::ToolResult => (theme::SUCCESS, " RESULT "),
    };

    lines.push(Line::from(Span::styled(
        header_text,
        Style::default().bg(header_bg).fg(theme::TEXT_BRIGHT).bold(),
    )));
    lines.push(Line::default());

    for line in msg.content.lines() {
        if line.trim().is_empty() {
            lines.push(Line::default());
        } else if line.starts_with("```") {
            lines.push(Line::from(Span::styled(line, Style::default().fg(theme::TEXT_DIM))));
        } else {
            lines.push(parse_inline_formatting(line));
        }
    }
    
    lines
}

fn parse_inline_formatting(line: &str) -> Line {
    let mut spans = Vec::new();
    let mut remaining = line;
    
    while !remaining.is_empty() {
        if let Some(pos) = remaining.find("**") {
            if pos > 0 {
                spans.push(Span::raw(&remaining[..pos]));
            }
            remaining = &remaining[pos + 2..];
            if let Some(end) = remaining.find("**") {
                spans.push(Span::styled(
                    &remaining[..end],
                    Style::default().add_modifier(Modifier::BOLD),
                ));
                remaining = &remaining[end + 2..];
            } else {
                spans.push(Span::raw("**"));
            }
        } else {
            spans.push(Span::raw(remaining));
            break;
        }
    }
    
    Line::from(spans).style(Style::default().fg(theme::TEXT))
}

fn render_status_bar(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let s = &app.status;
    
    let status = format!(
        " {} │ {} │ {} │ {} │ in:{} out:{} │ ${:.4} │ {:02}:{:02} ",
        app.model,
        &app.session_id[..8.min(app.session_id.len())],
        s.agent_profile,
        s.permission_mode,
        s.input_tokens,
        s.output_tokens,
        s.cost_usd,
        s.elapsed_secs / 60,
        s.elapsed_secs % 60,
    );

    let bar = Paragraph::new(Text::styled(status, Style::default().bg(theme::SURFACE).fg(theme::TEXT_DIM)))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme::BORDER))
        );

    frame.render_widget(bar, area);
}

fn render_input(frame: &mut ratatui::Frame, area: Rect, app: &App) {
    let prompt = Span::styled("❯ ", Style::default().fg(theme::USER_BG).bold());
    let text = if app.input_buffer.is_empty() {
        Span::styled("Type a message... (Tab: agent, Ctrl+C: cancel)", Style::default().fg(theme::TEXT_DIM))
    } else {
        Span::raw(&app.input_buffer)
    };
    
    let input_style = if app.input_buffer.is_empty() {
        Style::default().bg(theme::SURFACE).fg(theme::TEXT_DIM)
    } else {
        Style::default().bg(theme::SURFACE).fg(theme::TEXT)
    };

    let input = Paragraph::new(Text::from(Line::from(vec![prompt, text])))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme::BORDER))
        )
        .style(input_style);

    frame.render_widget(input, area);
}

/// Run the TUI event loop
pub async fn run_tui(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>, app: Arc<Mutex<App>>) -> anyhow::Result<String> {
    let mut input = String::new();
    
    loop {
        // Render
        {
            let app_lock = app.lock().unwrap();
            if !app_lock.is_running {
                break;
            }
            render(terminal, &app_lock)?;
        }

        // Poll for events
        match poll(std::time::Duration::from_millis(50)) {
            Ok(true) => {
                if let Ok(CrosstermEvent::Key(key_event)) = crossterm::event::read() {
                    let mut app_lock = app.lock().unwrap();
                    
                    match (key_event.code, key_event.modifiers) {
                        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                            app_lock.is_running = false;
                            break;
                        }
                        (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                            app_lock.messages.clear();
                            app_lock.show_welcome = true;
                        }
                        (KeyCode::Tab, _) => {
                            let profiles = ["@build", "@plan", "@review", "@fix", "@test"];
                            let current = &app_lock.status.agent_profile;
                            if let Some(idx) = profiles.iter().position(|p| *p == current) {
                                app_lock.status.agent_profile = profiles[(idx + 1) % profiles.len()].to_string();
                            }
                        }
                        (KeyCode::Char(c), _) => {
                            app_lock.input_buffer.push(c);
                            app_lock.cursor_position += 1;
                        }
                        (KeyCode::Backspace, _) => {
                            if !app_lock.input_buffer.is_empty() {
                                app_lock.input_buffer.pop();
                                app_lock.cursor_position = app_lock.cursor_position.saturating_sub(1);
                            }
                        }
                        (KeyCode::Enter, _) => {
                            input = app_lock.input_buffer.clone();
                            if !input.trim().is_empty() {
                                app_lock.add_message(Message {
                                    msg_type: MessageType::User,
                                    content: input.clone(),
                                    timestamp: Instant::now(),
                                    model: None,
                                    tool_name: None,
                                });
                            }
                            app_lock.input_buffer.clear();
                            app_lock.cursor_position = 0;
                            drop(app_lock);
                            break;
                        }
                        (KeyCode::Esc, _) => {
                            app_lock.is_running = false;
                            break;
                        }
                        (KeyCode::Up, _) => {
                            app_lock.scroll_offset = app_lock.scroll_offset.saturating_sub(1);
                        }
                        (KeyCode::Down, _) => {
                            let max = app_lock.messages.len().saturating_sub(1);
                            app_lock.scroll_offset = (app_lock.scroll_offset + 1).min(max);
                        }
                        _ => {}
                    }
                }
            }
            Ok(false) => {}
            Err(_) => break,
        }
    }
    
    Ok(input)
}