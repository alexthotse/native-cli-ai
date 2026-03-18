//! Full-screen session TUI: transcript, streaming assistant, composer.

use crate::slash_commands::SLASH_COMMANDS;
use crate::tui::state::{DisplayBlock, TuiSessionState};
use crossterm::{
    cursor::{Hide, MoveToColumn, Show},
    event::{
        DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseButton,
        MouseEventKind, poll, read,
    },
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};
use std::io::{stdout, Stdout};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub enum TuiCmd {
    Submit(String),
    CycleAgent,
    CancelTurn,
    Exit,
}

mod theme {
    use ratatui::style::Color;

    pub const BG: Color = Color::Rgb(22, 22, 28);
    pub const SURFACE: Color = Color::Rgb(32, 32, 42);
    pub const BORDER: Color = Color::Rgb(55, 55, 70);

    pub const USER: Color = Color::Rgb(56, 189, 248);
    pub const ASSISTANT: Color = Color::Rgb(167, 139, 250);
    pub const TOOL: Color = Color::Rgb(94, 234, 212);
    pub const MUTED: Color = Color::Rgb(120, 120, 140);
    pub const TEXT: Color = Color::Rgb(230, 230, 240);
    pub const SUCCESS: Color = Color::Rgb(74, 222, 128);
    pub const ERROR: Color = Color::Rgb(248, 113, 113);
    pub const WARN: Color = Color::Rgb(251, 191, 36);
}

const SLASH_PANEL_MAX_ROWS: usize = 8;
const MOUSE_SCROLL_LINES: usize = 3;

fn slash_panel_visible(buffer: &str) -> bool {
    buffer.starts_with('/') && !buffer.contains(' ')
}

fn filter_slash_commands(buffer: &str) -> Vec<&'static str> {
    if !slash_panel_visible(buffer) {
        return Vec::new();
    }
    SLASH_COMMANDS
        .iter()
        .copied()
        .filter(|c| c.starts_with(buffer))
        .collect()
}

fn slash_panel_height(filtered_len: usize) -> u16 {
    if filtered_len == 0 {
        return 0;
    }
    let rows = filtered_len.min(SLASH_PANEL_MAX_ROWS);
    let footer = if filtered_len > SLASH_PANEL_MAX_ROWS { 1 } else { 0 };
    // borders (2) + command rows + optional footer
    (rows as u16)
        .saturating_add(footer)
        .saturating_add(2)
        .min(14)
}

fn layout_chunks(area: Rect, slash_h: u16) -> (Rect, Rect, Option<Rect>, Rect) {
    if slash_h > 0 {
        let c = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(4),
                Constraint::Length(2),
                Constraint::Length(slash_h),
                Constraint::Length(3),
            ])
            .split(area);
        (c[0], c[1], Some(c[2]), c[3])
    } else {
        let c = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(4),
                Constraint::Length(2),
                Constraint::Length(3),
            ])
            .split(area);
        (c[0], c[1], None, c[2])
    }
}

fn rect_contains(r: Rect, col: u16, row: u16) -> bool {
    col >= r.x && col < r.x.saturating_add(r.width) && row >= r.y && row < r.y.saturating_add(r.height)
}

pub fn setup_terminal() -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode().map_err(|e| anyhow::anyhow!("enable_raw_mode: {e}"))?;
    let res: anyhow::Result<Terminal<CrosstermBackend<Stdout>>> = (|| {
        let mut out = stdout();
        execute!(out, EnterAlternateScreen)?;
        execute!(out, EnableMouseCapture)?;
        execute!(out, Hide)?;
        execute!(out, Clear(ClearType::All))?;
        Ok(Terminal::new(CrosstermBackend::new(out))?)
    })();
    if res.is_err() {
        let _ = disable_raw_mode();
    }
    res
}

pub fn restore_terminal() {
    let mut out = stdout();
    let _ = execute!(out, Show);
    let _ = execute!(out, DisableMouseCapture);
    let _ = execute!(out, LeaveAlternateScreen);
    let _ = disable_raw_mode();
}

/// Build scrollable transcript lines (for layout + scroll).
fn transcript_lines(state: &TuiSessionState, width: u16) -> Vec<Line<'static>> {
    let w = width.max(20) as usize;
    let mut lines: Vec<Line<'static>> = Vec::new();

    for block in &state.blocks {
        match block {
            DisplayBlock::User(content) => {
                lines.push(Line::from(vec![Span::styled(
                    " YOU ",
                    Style::default()
                        .fg(Color::Black)
                        .bg(theme::USER)
                        .add_modifier(Modifier::BOLD),
                )]));
                lines.push(Line::default());
                for text_line in wrap_text(content, w) {
                    lines.push(Line::from(Span::styled(text_line, Style::default().fg(theme::TEXT))));
                }
                lines.push(Line::default());
            }
            DisplayBlock::Assistant(content) => {
                lines.push(Line::from(vec![Span::styled(
                    " nca ",
                    Style::default()
                        .fg(Color::Black)
                        .bg(theme::ASSISTANT)
                        .add_modifier(Modifier::BOLD),
                )]));
                lines.push(Line::default());
                for text_line in wrap_text(content, w) {
                    lines.push(parse_md_line(&text_line));
                }
                lines.push(Line::default());
            }
            DisplayBlock::ToolRunning { name, .. } => {
                lines.push(Line::from(vec![
                    Span::styled(" ⚡ ", Style::default().fg(theme::TOOL)),
                    Span::styled(
                        format!("{name} "),
                        Style::default()
                            .fg(theme::TOOL)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("…", Style::default().fg(theme::MUTED)),
                ]));
            }
            DisplayBlock::ToolDone { name, ok, detail } => {
                let (icon, st) = if *ok {
                    ("✓", Style::default().fg(theme::SUCCESS))
                } else {
                    ("✗", Style::default().fg(theme::ERROR))
                };
                lines.push(Line::from(vec![
                    Span::styled(format!(" {icon} "), st),
                    Span::styled(
                        format!("{name}"),
                        Style::default().fg(theme::TOOL).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" — {}", truncate_chars(detail, 100)),
                        Style::default().fg(theme::MUTED),
                    ),
                ]));
            }
            DisplayBlock::System(s) => {
                lines.push(Line::from(Span::styled(
                    format!(" ‣ {s}"),
                    Style::default().fg(theme::WARN),
                )));
            }
            DisplayBlock::ErrorLine(s) => {
                lines.push(Line::from(Span::styled(
                    format!(" ✗ {s}"),
                    Style::default().fg(theme::ERROR),
                )));
            }
        }
    }

    if let Some(stream) = &state.streaming_assistant {
        if !stream.is_empty() {
            lines.push(Line::from(vec![
                Span::styled(" nca ", Style::default().fg(Color::Black).bg(theme::ASSISTANT)),
                Span::styled(" streaming", Style::default().fg(theme::MUTED)),
            ]));
            lines.push(Line::default());
            for text_line in wrap_text(stream, w) {
                lines.push(parse_md_line(&text_line));
            }
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("nca", Style::default().fg(theme::ASSISTANT).add_modifier(Modifier::BOLD)),
            Span::styled(" — session ready", Style::default().fg(theme::MUTED)),
        ]));
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(
            "Tab  agent   !cmd  shell   @path  search   /  commands   wheel  scroll",
            Style::default().fg(theme::MUTED),
        )));
    }

    lines
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max.saturating_sub(1)).collect::<String>())
    }
}

fn wrap_text(s: &str, width: usize) -> Vec<String> {
    if width < 8 {
        return vec![s.to_string()];
    }
    let mut out = Vec::new();
    for paragraph in s.split('\n') {
        if paragraph.is_empty() {
            out.push(String::new());
            continue;
        }
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            if line.is_empty() {
                line = word.to_string();
            } else if line.len() + 1 + word.len() <= width {
                line.push(' ');
                line.push_str(word);
            } else {
                out.push(line);
                line = word.to_string();
            }
        }
        if !line.is_empty() {
            out.push(line);
        }
    }
    if out.is_empty() && !s.is_empty() {
        out.push(s.to_string());
    }
    out
}

fn parse_md_line(line: &str) -> Line<'static> {
    if line.starts_with("```") {
        return Line::from(Span::styled(
            line.to_string(),
            Style::default().fg(theme::MUTED),
        ));
    }
    let mut spans: Vec<Span> = Vec::new();
    let mut rest = line.to_string();
    while !rest.is_empty() {
        if let Some(pos) = rest.find("**") {
            if pos > 0 {
                spans.push(Span::styled(
                    rest[..pos].to_string(),
                    Style::default().fg(theme::TEXT),
                ));
            }
            rest = rest[pos + 2..].to_string();
            if let Some(end) = rest.find("**") {
                spans.push(Span::styled(
                    rest[..end].to_string(),
                    Style::default()
                        .fg(theme::TEXT)
                        .add_modifier(Modifier::BOLD),
                ));
                rest = rest[end + 2..].to_string();
            } else {
                spans.push(Span::raw("**"));
                break;
            }
        } else {
            spans.push(Span::styled(rest, Style::default().fg(theme::TEXT)));
            break;
        }
    }
    Line::from(spans)
}

pub fn run_blocking(
    state: Arc<Mutex<TuiSessionState>>,
    cmd_tx: UnboundedSender<TuiCmd>,
    show_run_banner: bool,
) -> anyhow::Result<()> {
    let mut terminal = setup_terminal()?;

    if show_run_banner {
        if let Ok(mut g) = state.lock() {
            g.blocks.push(DisplayBlock::System(
                "Interactive run — type a message, Tab cycles agent profile.".into(),
            ));
        }
    }

    loop {
        {
            let mut g = state.lock().map_err(|e| anyhow::anyhow!("lock: {e}"))?;
            if g.should_exit {
                break;
            }

            let filtered = filter_slash_commands(&g.input_buffer);
            let slash_h = slash_panel_height(filtered.len());

            terminal.draw(|frame| {
                let area = frame.area();
                let (tr, st_r, slash_opt, inp_r) = layout_chunks(area, slash_h);

                let transcript_h = tr.height.saturating_sub(2) as usize;
                let lines = transcript_lines(&g, tr.width.saturating_sub(2));
                let total = lines.len();
                let max_scroll = total.saturating_sub(transcript_h);
                g.scroll_lines = g.scroll_lines.min(max_scroll);
                let start = g.scroll_lines;
                let end = (start + transcript_h).min(total);
                let visible: Vec<Line> = if start < end {
                    lines[start..end].to_vec()
                } else {
                    vec![]
                };

                let title = format!(
                    " transcript — {} lines (↑↓ wheel) ",
                    total
                );
                let main = Paragraph::new(Text::from(visible))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(theme::BORDER))
                            .title(Span::styled(title, Style::default().fg(theme::MUTED))),
                    )
                    .wrap(Wrap { trim: false })
                    .style(Style::default().bg(theme::BG));

                frame.render_widget(main, tr);

                let elapsed = g.started.elapsed().as_secs();
                let busy = if g.busy {
                    Span::styled(" ● busy ", Style::default().fg(theme::WARN))
                } else {
                    Span::styled(" ○ idle ", Style::default().fg(theme::SUCCESS))
                };
                let status = Line::from(vec![
                    busy,
                    Span::raw(" │ "),
                    Span::styled(&g.model, Style::default().fg(theme::USER)),
                    Span::raw(" │ "),
                    Span::styled(
                        format!("{}", &g.session_id[..8.min(g.session_id.len())]),
                        Style::default().fg(theme::MUTED),
                    ),
                    Span::raw(" │ "),
                    Span::styled(&g.agent_profile, Style::default().fg(theme::ASSISTANT)),
                    Span::raw(" │ "),
                    Span::styled(&g.permission_mode, Style::default().fg(theme::MUTED)),
                    Span::raw(" │ in:"),
                    Span::styled(format!("{}", g.input_tokens), Style::default().fg(theme::TEXT)),
                    Span::raw(" out:"),
                    Span::styled(format!("{}", g.output_tokens), Style::default().fg(theme::TEXT)),
                    Span::raw(" │ $"),
                    Span::styled(
                        format!("{:.4}", g.cost_usd),
                        Style::default().fg(theme::SUCCESS),
                    ),
                    Span::raw(" │ "),
                    Span::styled(
                        format!("{:02}:{:02}", elapsed / 60, elapsed % 60),
                        Style::default().fg(theme::MUTED),
                    ),
                ]);
                let bar = Paragraph::new(status).style(Style::default().bg(theme::SURFACE));
                frame.render_widget(bar, st_r);

                if let Some(sr) = slash_opt {
                    if !filtered.is_empty() {
                        let n_show = filtered.len().min(SLASH_PANEL_MAX_ROWS);
                        let max_scroll = filtered.len().saturating_sub(n_show);
                        let list_scroll = g.slash_menu_index.saturating_sub(n_show.saturating_sub(1)).min(max_scroll);
                        let mut slash_lines: Vec<Line> = Vec::new();
                        for (i, cmd) in filtered[list_scroll..list_scroll + n_show].iter().enumerate() {
                            let global = list_scroll + i;
                            let st = if global == g.slash_menu_index {
                                Style::default()
                                    .fg(Color::Black)
                                    .bg(theme::USER)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(theme::TEXT)
                            };
                            slash_lines.push(Line::from(Span::styled(format!(" {cmd}"), st)));
                        }
                        if filtered.len() > n_show {
                            slash_lines.push(Line::from(Span::styled(
                                format!(
                                    " ─ {}/{} · ↑↓",
                                    g.slash_menu_index + 1,
                                    filtered.len()
                                ),
                                Style::default().fg(theme::MUTED),
                            )));
                        }
                        let slash_w = Paragraph::new(Text::from(slash_lines))
                            .block(
                                Block::default()
                                    .borders(Borders::ALL)
                                    .border_style(Style::default().fg(theme::BORDER))
                                    .title(Span::styled(
                                        " commands (↑↓ Tab complete) ",
                                        Style::default().fg(theme::MUTED),
                                    )),
                            )
                            .style(Style::default().bg(theme::SURFACE));
                        frame.render_widget(slash_w, sr);
                    }
                }

                let prompt = Span::styled("❯ ", Style::default().fg(theme::USER).bold());
                let before: String = g.input_buffer.chars().take(g.cursor_char_idx).collect();
                let after: String = g.input_buffer.chars().skip(g.cursor_char_idx).collect();
                let input_line = Line::from(vec![
                    prompt,
                    Span::styled(before, Style::default().fg(theme::TEXT)),
                    Span::styled(
                        if after.is_empty() {
                            " ".into()
                        } else {
                            after.chars().next().map(|c| c.to_string()).unwrap_or_default()
                        },
                        Style::default()
                            .bg(theme::MUTED)
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        after.chars().skip(1).collect::<String>(),
                        Style::default().fg(theme::TEXT),
                    ),
                ]);

                let hint = if g.input_buffer.is_empty() {
                    Line::from(Span::styled(
                        "Enter send · Tab agent · / for commands · Esc exit · Ctrl+L clear",
                        Style::default().fg(theme::MUTED),
                    ))
                } else {
                    Line::default()
                };

                let input_block = Paragraph::new(Text::from(vec![input_line, hint]))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(theme::BORDER))
                            .title(Span::styled(" message ", Style::default().fg(theme::MUTED))),
                    )
                    .style(Style::default().bg(theme::SURFACE));

                frame.render_widget(input_block, inp_r);
            })?;
        }

        if poll(Duration::from_millis(40))? {
            let ev = read()?;
            let mut g = state.lock().map_err(|e| anyhow::anyhow!("lock: {e}"))?;

            match ev {
                Event::Mouse(m) => {
                    let sz = terminal.size()?;
                    let area = Rect::new(0, 0, sz.width, sz.height);
                    let filtered = filter_slash_commands(&g.input_buffer);
                    let sh = slash_panel_height(filtered.len());
                    let (tr, _, slash_r, _) = layout_chunks(area, sh);

                    if rect_contains(tr, m.column, m.row) {
                        let lines = transcript_lines(&g, tr.width.saturating_sub(2));
                        let total = lines.len();
                        let th = tr.height.saturating_sub(2) as usize;
                        let max_scroll = total.saturating_sub(th);
                        match m.kind {
                            MouseEventKind::ScrollUp => {
                                g.scroll_lines = g.scroll_lines.saturating_sub(MOUSE_SCROLL_LINES);
                            }
                            MouseEventKind::ScrollDown => {
                                g.scroll_lines =
                                    (g.scroll_lines + MOUSE_SCROLL_LINES).min(max_scroll);
                            }
                            _ => {}
                        }
                    }

                    if let Some(sr) = slash_r {
                        if rect_contains(sr, m.column, m.row)
                            && matches!(m.kind, MouseEventKind::Down(MouseButton::Left))
                        {
                            let inner_y = m.row.saturating_sub(sr.y).saturating_sub(1);
                            let n_show = filtered.len().min(SLASH_PANEL_MAX_ROWS);
                            let max_scroll = filtered.len().saturating_sub(n_show);
                            let list_scroll = g.slash_menu_index.saturating_sub(n_show.saturating_sub(1)).min(max_scroll);
                            if (inner_y as usize) < n_show {
                                let idx = list_scroll + inner_y as usize;
                                if idx < filtered.len() {
                                    g.input_buffer = filtered[idx].to_string();
                                    g.cursor_char_idx = g.input_buffer.chars().count();
                                    g.slash_menu_index = idx;
                                }
                            }
                        }
                    }
                }
                Event::Key(key) => match (key.code, key.modifiers) {
                    (KeyCode::Esc, _) => {
                        g.should_exit = true;
                        let _ = cmd_tx.send(TuiCmd::Exit);
                        break;
                    }
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        let _ = cmd_tx.send(TuiCmd::CancelTurn);
                    }
                    (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                        g.blocks.clear();
                        g.streaming_assistant = None;
                        g.scroll_lines = 0;
                    }
                    (KeyCode::Tab, _) => {
                        let filtered = filter_slash_commands(&g.input_buffer);
                        if !filtered.is_empty() && slash_panel_visible(&g.input_buffer) {
                            let pick = g.slash_menu_index % filtered.len();
                            g.input_buffer = filtered[pick].to_string();
                            g.cursor_char_idx = g.input_buffer.chars().count();
                        } else {
                            drop(g);
                            let _ = cmd_tx.send(TuiCmd::CycleAgent);
                        }
                    }
                    (KeyCode::Enter, _) => {
                        let line = std::mem::take(&mut g.input_buffer);
                        g.cursor_char_idx = 0;
                        g.slash_menu_index = 0;
                        drop(g);
                        let _ = cmd_tx.send(TuiCmd::Submit(line));
                    }
                    (KeyCode::Char('a'), KeyModifiers::CONTROL) | (KeyCode::Home, _) => {
                        g.cursor_char_idx = 0;
                    }
                    (KeyCode::Char('e'), KeyModifiers::CONTROL) | (KeyCode::End, _) => {
                        g.cursor_char_idx = g.input_buffer.chars().count();
                    }
                    (KeyCode::Left, _) => {
                        g.cursor_char_idx = g.cursor_char_idx.saturating_sub(1);
                    }
                    (KeyCode::Right, _) => {
                        let max = g.input_buffer.chars().count();
                        g.cursor_char_idx = (g.cursor_char_idx + 1).min(max);
                    }
                    (KeyCode::Up, _) => {
                        let filtered = filter_slash_commands(&g.input_buffer);
                        if !filtered.is_empty() && slash_panel_visible(&g.input_buffer) {
                            g.slash_menu_index = g.slash_menu_index.saturating_sub(1);
                        } else {
                            g.scroll_lines = g.scroll_lines.saturating_sub(1);
                        }
                    }
                    (KeyCode::Down, _) => {
                        let filtered = filter_slash_commands(&g.input_buffer);
                        if !filtered.is_empty() && slash_panel_visible(&g.input_buffer) {
                            let n = filtered.len();
                            g.slash_menu_index = (g.slash_menu_index + 1) % n;
                        } else {
                            let sz = terminal.size().ok();
                            if let Some(sz) = sz {
                                let area = Rect::new(0, 0, sz.width, sz.height);
                                let sh = slash_panel_height(filter_slash_commands(&g.input_buffer).len());
                                let (tr, _, _, _) = layout_chunks(area, sh);
                                let lines = transcript_lines(&g, tr.width.saturating_sub(2));
                                let total = lines.len();
                                let th = tr.height.saturating_sub(2) as usize;
                                let max_scroll = total.saturating_sub(th);
                                g.scroll_lines = (g.scroll_lines + 1).min(max_scroll);
                            }
                        }
                    }
                    (KeyCode::Backspace, _) => {
                        if g.cursor_char_idx > 0 {
                            let idx = g.cursor_char_idx;
                            let mut cs: Vec<char> = g.input_buffer.chars().collect();
                            cs.remove(idx - 1);
                            g.input_buffer = cs.into_iter().collect();
                            g.cursor_char_idx -= 1;
                            if slash_panel_visible(&g.input_buffer) {
                                let f = filter_slash_commands(&g.input_buffer);
                                if !f.is_empty() {
                                    g.slash_menu_index =
                                        g.slash_menu_index.min(f.len().saturating_sub(1));
                                } else {
                                    g.slash_menu_index = 0;
                                }
                            }
                        }
                    }
                    (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                        let idx = g.cursor_char_idx;
                        let mut cs: Vec<char> = g.input_buffer.chars().collect();
                        cs.insert(idx, c);
                        g.input_buffer = cs.into_iter().collect();
                        g.cursor_char_idx += 1;
                        if slash_panel_visible(&g.input_buffer) {
                            let f = filter_slash_commands(&g.input_buffer);
                            if !f.is_empty() {
                                g.slash_menu_index =
                                    g.slash_menu_index.min(f.len().saturating_sub(1));
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    restore_terminal();
    let _ = execute!(stdout(), MoveToColumn(0));
    Ok(())
}
