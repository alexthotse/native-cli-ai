//! Full-screen session TUI: transcript, streaming assistant, composer.

use crate::slash_commands::SLASH_COMMANDS;
use crate::tui::state::{ApprovalRequest, DisplayBlock, TuiSessionState};
use crossterm::{
    cursor::{Hide, MoveToColumn, Show},
    event::{
        DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseButton,
        MouseEventKind, poll, read,
    },
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use nca_common::event::QuestionSelection;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear as ClearWidget, Paragraph, Wrap},
};
use std::io::{Stdout, stdout};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

/// Per flattened transcript line: click selects this answer (same indices as `transcript_lines`).
type LineAnswerHit = Option<QuestionSelection>;

#[derive(Debug)]
pub enum TuiCmd {
    Submit(String),
    /// Answer for the current `ask_question` (from question mode or `/auto-answer`).
    QuestionAnswer(nca_common::event::QuestionSelection),
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
const SIDEBAR_WIDTH: u16 = 32;
const SIDEBAR_MIN_TOTAL_WIDTH: u16 = 110;
const COMMAND_PALETTE_WIDTH: u16 = 48;
const COMMAND_PALETTE_MAX_ROWS: usize = 10;

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

fn filter_command_palette(query: &str) -> Vec<&'static str> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return SLASH_COMMANDS.to_vec();
    }
    let needle = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };
    SLASH_COMMANDS
        .iter()
        .copied()
        .filter(|c| c.starts_with(&needle))
        .collect()
}

fn slash_panel_height(filtered_len: usize) -> u16 {
    if filtered_len == 0 {
        return 0;
    }
    let rows = filtered_len.min(SLASH_PANEL_MAX_ROWS);
    let footer = if filtered_len > SLASH_PANEL_MAX_ROWS {
        1
    } else {
        0
    };
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

fn sidebar_fit(s: &str, max_chars: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max_chars {
        t.to_string()
    } else {
        format!(
            "{}…",
            t.chars()
                .take(max_chars.saturating_sub(1))
                .collect::<String>()
        )
    }
}

fn layout_with_sidebar(area: Rect) -> (Rect, Option<Rect>) {
    if area.width < SIDEBAR_MIN_TOTAL_WIDTH {
        return (area, None);
    }
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(60), Constraint::Length(SIDEBAR_WIDTH)])
        .split(area);
    (chunks[0], Some(chunks[1]))
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let popup_w = width
        .min(area.width.saturating_sub(2).max(20))
        .min(area.width);
    let popup_h = height
        .min(area.height.saturating_sub(2).max(6))
        .min(area.height);
    Rect::new(
        area.x + area.width.saturating_sub(popup_w) / 2,
        area.y + area.height.saturating_sub(popup_h) / 2,
        popup_w,
        popup_h,
    )
}

/// Matches `PermissionMode` as stored via `format!("{:?}", mode)` (e.g. `BypassPermissions`).
fn toolbar_permission_is_bypass(mode: &str) -> bool {
    mode.contains("BypassPermissions")
}

fn rect_contains(r: Rect, col: u16, row: u16) -> bool {
    col >= r.x
        && col < r.x.saturating_add(r.width)
        && row >= r.y
        && row < r.y.saturating_add(r.height)
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

#[inline]
fn push_transcript_line(
    lines: &mut Vec<Line<'static>>,
    hits: &mut Vec<LineAnswerHit>,
    line: Line<'static>,
    hit: LineAnswerHit,
) {
    lines.push(line);
    hits.push(hit);
}

/// Build scrollable transcript lines + optional mouse/click targets per line.
fn transcript_lines_and_hits(
    state: &TuiSessionState,
    width: u16,
) -> (Vec<Line<'static>>, Vec<LineAnswerHit>) {
    let w = width.max(20) as usize;
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut hits: Vec<LineAnswerHit> = Vec::new();

    for block in &state.blocks {
        match block {
            DisplayBlock::User(content) => {
                push_transcript_line(
                    &mut lines,
                    &mut hits,
                    Line::from(vec![Span::styled(
                        " YOU ",
                        Style::default()
                            .fg(Color::Black)
                            .bg(theme::USER)
                            .add_modifier(Modifier::BOLD),
                    )]),
                    None,
                );
                push_transcript_line(&mut lines, &mut hits, Line::default(), None);
                for text_line in wrap_text(content, w) {
                    push_transcript_line(
                        &mut lines,
                        &mut hits,
                        Line::from(Span::styled(text_line, Style::default().fg(theme::TEXT))),
                        None,
                    );
                }
                push_transcript_line(&mut lines, &mut hits, Line::default(), None);
            }
            DisplayBlock::Assistant(content) => {
                push_transcript_line(
                    &mut lines,
                    &mut hits,
                    Line::from(vec![Span::styled(
                        " nca ",
                        Style::default()
                            .fg(Color::Black)
                            .bg(theme::ASSISTANT)
                            .add_modifier(Modifier::BOLD),
                    )]),
                    None,
                );
                push_transcript_line(&mut lines, &mut hits, Line::default(), None);
                for text_line in wrap_text(content, w) {
                    push_transcript_line(&mut lines, &mut hits, parse_md_line(&text_line), None);
                }
                push_transcript_line(&mut lines, &mut hits, Line::default(), None);
            }
            DisplayBlock::ToolRunning { name, .. } => {
                push_transcript_line(
                    &mut lines,
                    &mut hits,
                    Line::from(vec![
                        Span::styled(" ⚡ ", Style::default().fg(theme::TOOL)),
                        Span::styled(
                            format!("{name} "),
                            Style::default()
                                .fg(theme::TOOL)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled("…", Style::default().fg(theme::MUTED)),
                    ]),
                    None,
                );
            }
            DisplayBlock::ApprovalPending(req) => {
                render_approval_block(&mut lines, &mut hits, req, w);
            }
            DisplayBlock::ApprovalResolved { tool, approved } => {
                let (label, style) = if *approved {
                    (
                        " approved ",
                        Style::default().fg(Color::Black).bg(theme::SUCCESS),
                    )
                } else {
                    (
                        " denied ",
                        Style::default().fg(Color::Black).bg(theme::ERROR),
                    )
                };
                push_transcript_line(
                    &mut lines,
                    &mut hits,
                    Line::from(vec![
                        Span::styled(label, style.add_modifier(Modifier::BOLD)),
                        Span::styled(format!(" {tool}"), Style::default().fg(theme::TEXT)),
                    ]),
                    None,
                );
                push_transcript_line(&mut lines, &mut hits, Line::default(), None);
            }
            DisplayBlock::ToolDone { name, ok, detail } => {
                let (icon, st) = if *ok {
                    ("✓", Style::default().fg(theme::SUCCESS))
                } else {
                    ("✗", Style::default().fg(theme::ERROR))
                };
                push_transcript_line(
                    &mut lines,
                    &mut hits,
                    Line::from(vec![
                        Span::styled(format!(" {icon} "), st),
                        Span::styled(
                            format!("{name}"),
                            Style::default()
                                .fg(theme::TOOL)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!(" — {}", truncate_chars(detail, 100)),
                            Style::default().fg(theme::MUTED),
                        ),
                    ]),
                    None,
                );
            }
            DisplayBlock::System(s) => {
                push_transcript_line(
                    &mut lines,
                    &mut hits,
                    Line::from(Span::styled(
                        format!(" ‣ {s}"),
                        Style::default().fg(theme::WARN),
                    )),
                    None,
                );
            }
            DisplayBlock::Question(q) => {
                push_transcript_line(
                    &mut lines,
                    &mut hits,
                    Line::from(vec![
                        Span::styled(
                            " ? ",
                            Style::default().fg(Color::Black).bg(theme::WARN).bold(),
                        ),
                        Span::styled(
                            " question ",
                            Style::default()
                                .fg(theme::WARN)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    None,
                );
                push_transcript_line(&mut lines, &mut hits, Line::default(), None);
                for text_line in wrap_text(&q.prompt, w) {
                    push_transcript_line(
                        &mut lines,
                        &mut hits,
                        Line::from(Span::styled(text_line, Style::default().fg(theme::TEXT))),
                        None,
                    );
                }
                push_transcript_line(
                    &mut lines,
                    &mut hits,
                    Line::from(vec![
                        Span::styled(
                            format!("  [0] suggested: {} ", q.suggested_answer),
                            Style::default()
                                .fg(theme::SUCCESS)
                                .add_modifier(Modifier::UNDERLINED),
                        ),
                        Span::styled("(click)", Style::default().fg(theme::MUTED)),
                    ]),
                    Some(QuestionSelection::Suggested),
                );
                for (i, o) in q.options.iter().enumerate() {
                    push_transcript_line(
                        &mut lines,
                        &mut hits,
                        Line::from(vec![
                            Span::styled(
                                format!("  [{}] ({}) {} ", i + 1, o.id, o.label),
                                Style::default()
                                    .fg(theme::TEXT)
                                    .add_modifier(Modifier::UNDERLINED),
                            ),
                            Span::styled("(click)", Style::default().fg(theme::MUTED)),
                        ]),
                        Some(QuestionSelection::Option {
                            option_id: o.id.clone(),
                        }),
                    );
                }
                if q.allow_custom {
                    push_transcript_line(
                        &mut lines,
                        &mut hits,
                        Line::from(Span::styled(
                            "  [c] type your own answer below, then Enter",
                            Style::default().fg(theme::MUTED),
                        )),
                        None,
                    );
                }
                push_transcript_line(
                    &mut lines,
                    &mut hits,
                    Line::from(Span::styled(
                        "  Tip: /auto-answer or Enter on empty = suggested · click an option above",
                        Style::default().fg(theme::MUTED),
                    )),
                    None,
                );
                push_transcript_line(&mut lines, &mut hits, Line::default(), None);
            }
            DisplayBlock::ErrorLine(s) => {
                push_transcript_line(
                    &mut lines,
                    &mut hits,
                    Line::from(Span::styled(
                        format!(" ✗ {s}"),
                        Style::default().fg(theme::ERROR),
                    )),
                    None,
                );
            }
        }
    }

    if let Some(stream) = &state.streaming_assistant {
        if !stream.is_empty() {
            push_transcript_line(
                &mut lines,
                &mut hits,
                Line::from(vec![
                    Span::styled(
                        " nca ",
                        Style::default().fg(Color::Black).bg(theme::ASSISTANT),
                    ),
                    Span::styled(" streaming", Style::default().fg(theme::MUTED)),
                ]),
                None,
            );
            push_transcript_line(&mut lines, &mut hits, Line::default(), None);
            for text_line in wrap_text(stream, w) {
                push_transcript_line(&mut lines, &mut hits, parse_md_line(&text_line), None);
            }
        }
    }

    if lines.is_empty() {
        push_transcript_line(
            &mut lines,
            &mut hits,
            Line::from(vec![
                Span::styled(
                    "nca",
                    Style::default()
                        .fg(theme::ASSISTANT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" — session ready", Style::default().fg(theme::MUTED)),
            ]),
            None,
        );
        push_transcript_line(&mut lines, &mut hits, Line::default(), None);
        push_transcript_line(
            &mut lines,
            &mut hits,
            Line::from(Span::styled(
                "Tab  agent   Ctrl+P  commands   !cmd  shell   @path  search   /  inline   wheel  scroll",
                Style::default().fg(theme::MUTED),
            )),
            None,
        );
    }

    (lines, hits)
}

fn transcript_lines(state: &TuiSessionState, width: u16) -> Vec<Line<'static>> {
    transcript_lines_and_hits(state, width).0
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!(
            "{}…",
            s.chars().take(max.saturating_sub(1)).collect::<String>()
        )
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

fn wrap_preformatted_line(line: &str, width: usize) -> Vec<String> {
    if width < 4 || line.is_empty() {
        return vec![line.to_string()];
    }
    let mut out = Vec::new();
    let mut current = String::new();
    let mut current_len = 0usize;
    for ch in line.chars() {
        if current_len >= width {
            out.push(current);
            current = String::new();
            current_len = 0;
        }
        current.push(ch);
        current_len += 1;
    }
    if out.is_empty() || !current.is_empty() {
        out.push(current);
    }
    out
}

fn push_wrapped_plain_lines(
    lines: &mut Vec<Line<'static>>,
    hits: &mut Vec<LineAnswerHit>,
    text: &str,
    width: usize,
    style: Style,
) {
    for source_line in text.lines() {
        let wrapped = wrap_preformatted_line(source_line, width);
        for line in wrapped {
            push_transcript_line(lines, hits, Line::from(Span::styled(line, style)), None);
        }
        if source_line.is_empty() {
            push_transcript_line(lines, hits, Line::default(), None);
        }
    }
}

fn render_approval_block(
    lines: &mut Vec<Line<'static>>,
    hits: &mut Vec<LineAnswerHit>,
    req: &ApprovalRequest,
    width: usize,
) {
    push_transcript_line(
        lines,
        hits,
        Line::from(vec![
            Span::styled(
                " ? ",
                Style::default().fg(Color::Black).bg(theme::WARN).bold(),
            ),
            Span::styled(
                format!(" approval required: {}", req.tool),
                Style::default()
                    .fg(theme::WARN)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        None,
    );
    push_transcript_line(lines, hits, Line::default(), None);
    for text_line in wrap_text(&req.description, width) {
        push_transcript_line(
            lines,
            hits,
            Line::from(Span::styled(text_line, Style::default().fg(theme::TEXT))),
            None,
        );
    }
    push_transcript_line(lines, hits, Line::default(), None);
    push_transcript_line(
        lines,
        hits,
        Line::from(Span::styled(
            " Input ",
            Style::default()
                .fg(theme::MUTED)
                .add_modifier(Modifier::BOLD),
        )),
        None,
    );
    push_wrapped_plain_lines(
        lines,
        hits,
        &req.input,
        width,
        Style::default().fg(theme::MUTED),
    );
    push_transcript_line(
        lines,
        hits,
        Line::from(Span::styled(
            " Reply with y/yes or n/no, then press Enter.",
            Style::default().fg(theme::MUTED),
        )),
        None,
    );
    push_transcript_line(lines, hits, Line::default(), None);
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

fn parse_tui_question_answer(
    raw: &str,
    q: &nca_common::event::InteractiveQuestionPayload,
) -> Option<QuestionSelection> {
    let t = raw.trim();
    if t.is_empty() || t == "0" || t.eq_ignore_ascii_case("s") {
        return Some(QuestionSelection::Suggested);
    }
    if let Ok(n) = t.parse::<usize>() {
        if n >= 1 && n <= q.options.len() {
            return Some(QuestionSelection::Option {
                option_id: q.options[n - 1].id.clone(),
            });
        }
    }
    if q.allow_custom && !t.is_empty() {
        return Some(QuestionSelection::Custom {
            text: t.to_string(),
        });
    }
    None
}

/// `question_answer_tx`: when `Some`, answers are sent there so they unblock `ask_question` while
/// the async loop is stuck in `run_turn` (that task does not poll `cmd_rx` until the turn ends).
pub fn run_blocking(
    state: Arc<Mutex<TuiSessionState>>,
    cmd_tx: UnboundedSender<TuiCmd>,
    question_answer_tx: Option<UnboundedSender<(String, QuestionSelection)>>,
    approval_answer_tx: Option<UnboundedSender<(String, bool)>>,
    show_run_banner: bool,
) -> anyhow::Result<()> {
    let mut terminal = setup_terminal()?;

    if show_run_banner {
        if let Ok(mut g) = state.lock() {
            g.blocks.push(DisplayBlock::System(
                "Interactive run — type a message, Tab cycles agent profile, Ctrl+P opens commands.".into(),
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
                let (main_area, sidebar_opt) = layout_with_sidebar(area);
                let (tr, st_r, slash_opt, inp_r) = layout_chunks(main_area, slash_h);

                let transcript_h = tr.height.saturating_sub(2) as usize;
                let inner_w = tr.width.saturating_sub(2);
                let (lines, _hits) = transcript_lines_and_hits(&g, inner_w);
                let total = lines.len();
                let max_scroll = total.saturating_sub(transcript_h);
                if g.transcript_follow_tail {
                    g.scroll_lines = max_scroll;
                } else {
                    g.scroll_lines = g.scroll_lines.min(max_scroll);
                }
                let start = g.scroll_lines;
                let end = (start + transcript_h).min(total);
                let visible: Vec<Line> = if start < end {
                    lines[start..end].to_vec()
                } else {
                    vec![]
                };

                let title = format!(
                    " transcript — {} lines (↑↓ wheel · End bottom) ",
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

                if let Some(sidebar) = sidebar_opt {
                    let sections = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(12),
                            Constraint::Length(8),
                            Constraint::Min(10),
                        ])
                        .split(sidebar);

                    let ws_line = if g.workspace_display.is_empty() {
                        "—".to_string()
                    } else {
                        sidebar_fit(&g.workspace_display, 26)
                    };
                    let session_lines = vec![
                        Line::from(Span::styled(
                            "workspace",
                            Style::default().fg(theme::MUTED),
                        )),
                        Line::from(ws_line),
                        Line::default(),
                        Line::from(format!("session {}", &g.session_id[..8.min(g.session_id.len())])),
                        Line::from(format!("model   {}", g.model)),
                        Line::from(format!("agent   {}", g.agent_profile)),
                        Line::from(format!("mode    {}", g.permission_mode)),
                        Line::from(format!(
                            "status  {}",
                            if g.busy { "busy" } else { "idle" }
                        )),
                        Line::from(format!("blocks  {}", g.blocks.len())),
                        Line::from(format!("lines   {total}")),
                    ];
                    let session_block = Paragraph::new(Text::from(session_lines))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(theme::BORDER))
                                .title(Span::styled(
                                    " context ",
                                    Style::default().fg(theme::MUTED),
                                )),
                        )
                        .style(Style::default().bg(theme::SURFACE))
                        .wrap(Wrap { trim: false });
                    frame.render_widget(session_block, sections[0]);

                    let usage_lines = vec![
                        Line::from(format!("input   {}", g.input_tokens)),
                        Line::from(format!("output  {}", g.output_tokens)),
                        Line::from(format!("total   {}", g.input_tokens + g.output_tokens)),
                        Line::from(format!("cost    ${:.4}", g.cost_usd)),
                        Line::default(),
                        Line::from(if g.active_approval.is_some() {
                            "pending approval"
                        } else if g.active_question.is_some() {
                            "pending question"
                        } else {
                            "no pending prompt"
                        }),
                    ];
                    let usage_block = Paragraph::new(Text::from(usage_lines))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(theme::BORDER))
                                .title(Span::styled(
                                    " usage ",
                                    Style::default().fg(theme::MUTED),
                                )),
                        )
                        .style(Style::default().bg(theme::SURFACE))
                        .wrap(Wrap { trim: false });
                    frame.render_widget(usage_block, sections[1]);

                    let mut todo_lines: Vec<Line> = vec![Line::from(Span::styled(
                        "sub-agents",
                        Style::default()
                            .fg(theme::MUTED)
                            .add_modifier(Modifier::BOLD),
                    ))];
                    if g.subagents.is_empty() {
                        todo_lines.push(Line::from(Span::styled(
                            "none (spawn shows here)",
                            Style::default().fg(theme::MUTED),
                        )));
                    } else {
                        for row in g.subagents.iter().take(8) {
                            let dot = if row.running { "●" } else { "○" };
                            let id8 = sidebar_fit(&row.id, 8);
                            let ph = sidebar_fit(&row.phase, 11);
                            todo_lines.push(Line::from(vec![
                                Span::styled(
                                    format!("{dot} "),
                                    Style::default().fg(if row.running {
                                        theme::WARN
                                    } else {
                                        theme::MUTED
                                    }),
                                ),
                                Span::styled(format!("{id8} "), Style::default().fg(theme::TEXT)),
                                Span::styled(ph, Style::default().fg(theme::TOOL)),
                            ]));
                            if !row.detail.is_empty() {
                                todo_lines.push(Line::from(Span::styled(
                                    format!("  {}", sidebar_fit(&row.detail, 26)),
                                    Style::default().fg(theme::MUTED),
                                )));
                            }
                            if !row.task.is_empty() && row.task != "(sub-agent)" {
                                todo_lines.push(Line::from(Span::styled(
                                    format!("  {}", sidebar_fit(&row.task, 26)),
                                    Style::default().fg(theme::TEXT),
                                )));
                            }
                        }
                    }
                    todo_lines.push(Line::default());
                    todo_lines.push(Line::from(Span::styled(
                        "dev",
                        Style::default()
                            .fg(theme::MUTED)
                            .add_modifier(Modifier::BOLD),
                    )));
                    todo_lines.push(Line::from(Span::styled(
                        ".nca/sessions",
                        Style::default().fg(theme::USER),
                    )));
                    todo_lines.push(Line::from(Span::styled(
                        "docs/research/",
                        Style::default().fg(theme::USER),
                    )));
                    todo_lines.push(Line::from(Span::styled(
                        "Ctrl+P commands",
                        Style::default().fg(theme::MUTED),
                    )));
                    let todo_block = Paragraph::new(Text::from(todo_lines))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(theme::BORDER))
                                .title(Span::styled(
                                    " sidebar ",
                                    Style::default().fg(theme::MUTED),
                                )),
                        )
                        .style(Style::default().bg(theme::SURFACE))
                        .wrap(Wrap { trim: false });
                    frame.render_widget(todo_block, sections[2]);
                }

                let elapsed = g.started.elapsed().as_secs();
                let busy = if g.busy {
                    Span::styled(" ● busy ", Style::default().fg(theme::WARN))
                } else {
                    Span::styled(" ○ idle ", Style::default().fg(theme::SUCCESS))
                };
                let approval_hint = if g.active_approval.is_some() {
                    Span::styled(" !approve ", Style::default().fg(theme::ERROR))
                } else {
                    Span::raw("")
                };
                let q_hint = if g.active_question.is_some() {
                    Span::styled(" ?answer ", Style::default().fg(theme::WARN))
                } else {
                    Span::raw("")
                };
                // Session / tokens / cost live in the sidebar; keep the bar short and obvious about bypass.
                let perm_span = if toolbar_permission_is_bypass(&g.permission_mode) {
                    Span::styled(
                        " BYPASS — tools run without approval ",
                        Style::default()
                            .fg(Color::Black)
                            .bg(theme::ERROR)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::styled(
                        format!(" perm:{} ", g.permission_mode),
                        Style::default().fg(theme::MUTED),
                    )
                };
                let time_span = Span::styled(
                    format!("{:02}:{:02}", elapsed / 60, elapsed % 60),
                    Style::default().fg(theme::MUTED),
                );
                let mut status_spans = vec![
                    busy,
                    approval_hint,
                    q_hint,
                    Span::raw(" │ "),
                    Span::styled(&g.model, Style::default().fg(theme::USER)),
                    Span::raw(" │ "),
                    Span::styled(&g.agent_profile, Style::default().fg(theme::ASSISTANT)),
                    Span::raw(" │ "),
                    perm_span,
                ];
                // Sidebar is hidden on narrow terminals — put session/tokens/cost back on the bar.
                if sidebar_opt.is_none() {
                    status_spans.push(Span::raw(" │ "));
                    status_spans.push(Span::styled(
                        format!("{}", &g.session_id[..8.min(g.session_id.len())]),
                        Style::default().fg(theme::MUTED),
                    ));
                    status_spans.extend([
                        Span::raw(" │ in:"),
                        Span::styled(
                            format!("{}", g.input_tokens),
                            Style::default().fg(theme::TEXT),
                        ),
                        Span::raw(" out:"),
                        Span::styled(
                            format!("{}", g.output_tokens),
                            Style::default().fg(theme::TEXT),
                        ),
                        Span::raw(" │ $"),
                        Span::styled(
                            format!("{:.4}", g.cost_usd),
                            Style::default().fg(theme::SUCCESS),
                        ),
                    ]);
                }
                status_spans.push(Span::raw(" │ "));
                status_spans.push(time_span);
                let status = Line::from(status_spans);
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

                let hint = if g.active_approval.is_some() {
                    Line::from(Span::styled(
                        "Approval pending: type y/yes to approve or n/no to deny, then press Enter",
                        Style::default().fg(theme::ERROR),
                    ))
                } else if g.active_question.is_some() {
                    Line::from(Span::styled(
                        "Enter / 0 = suggested · 1–n = option · click underlined line · /auto-answer · End = transcript bottom (empty input)",
                        Style::default().fg(theme::WARN),
                    ))
                } else if g.input_buffer.is_empty() {
                    Line::from(Span::styled(
                        "Enter send · Tab agent · / inline commands · Ctrl+P command palette · Esc exit · Ctrl+L clear",
                        Style::default().fg(theme::MUTED),
                    ))
                } else {
                    Line::default()
                };

                let input_title = if g.active_approval.is_some() {
                    " approval "
                } else if g.active_question.is_some() {
                    " answer "
                } else {
                    " message "
                };
                let input_block = Paragraph::new(Text::from(vec![input_line, hint]))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(theme::BORDER))
                            .title(Span::styled(input_title, Style::default().fg(theme::MUTED))),
                    )
                    .style(Style::default().bg(theme::SURFACE));

                frame.render_widget(input_block, inp_r);

                if g.command_palette_open {
                    let filtered = filter_command_palette(&g.command_palette_query);
                    let rows = filtered.len().clamp(1, COMMAND_PALETTE_MAX_ROWS) as u16;
                    let popup_area =
                        centered_rect(area, COMMAND_PALETTE_WIDTH, rows.saturating_add(6));
                    let pick = g
                        .slash_menu_index
                        .min(filtered.len().saturating_sub(1));
                    let list_scroll = pick.saturating_sub(COMMAND_PALETTE_MAX_ROWS / 2);
                    let list_end = (list_scroll + COMMAND_PALETTE_MAX_ROWS).min(filtered.len());
                    let mut popup_lines = vec![
                        Line::from(vec![
                            Span::styled(
                                " Search ",
                                Style::default()
                                    .fg(theme::MUTED)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                if g.command_palette_query.is_empty() {
                                    "type to filter"
                                } else {
                                    g.command_palette_query.as_str()
                                },
                                Style::default().fg(theme::TEXT),
                            ),
                        ]),
                        Line::default(),
                    ];
                    if filtered.is_empty() {
                        popup_lines.push(Line::from(Span::styled(
                            " No matching commands",
                            Style::default().fg(theme::MUTED),
                        )));
                    } else {
                        for (idx, cmd) in filtered[list_scroll..list_end].iter().enumerate() {
                            let global = list_scroll + idx;
                            let style = if global == pick {
                                Style::default()
                                    .fg(Color::Black)
                                    .bg(theme::USER)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(theme::TEXT)
                            };
                            popup_lines.push(Line::from(Span::styled(format!(" {cmd}"), style)));
                        }
                    }
                    popup_lines.push(Line::default());
                    popup_lines.push(Line::from(Span::styled(
                        " Enter apply · Esc close ",
                        Style::default().fg(theme::MUTED),
                    )));
                    frame.render_widget(ClearWidget, popup_area);
                    let popup = Paragraph::new(Text::from(popup_lines))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(theme::BORDER))
                                .title(Span::styled(
                                    " commands ",
                                    Style::default().fg(theme::MUTED),
                                )),
                        )
                        .style(Style::default().bg(theme::SURFACE))
                        .wrap(Wrap { trim: false });
                    frame.render_widget(popup, popup_area);
                }
            })?;
        }

        if poll(Duration::from_millis(40))? {
            let ev = read()?;
            let mut g = state.lock().map_err(|e| anyhow::anyhow!("lock: {e}"))?;

            match ev {
                Event::Mouse(_) if g.command_palette_open => continue,
                Event::Mouse(m) => {
                    let sz = terminal.size()?;
                    let area = Rect::new(0, 0, sz.width, sz.height);
                    let (main_area, _) = layout_with_sidebar(area);
                    let filtered = filter_slash_commands(&g.input_buffer);
                    let sh = slash_panel_height(filtered.len());
                    let (tr, _, slash_r, _) = layout_chunks(main_area, sh);

                    if rect_contains(tr, m.column, m.row) {
                        let inner_w = tr.width.saturating_sub(2);
                        let (lines, hits) = transcript_lines_and_hits(&g, inner_w);
                        let total = lines.len();
                        let th = tr.height.saturating_sub(2) as usize;
                        let max_scroll = total.saturating_sub(th);
                        match m.kind {
                            MouseEventKind::ScrollUp => {
                                g.transcript_follow_tail = false;
                                g.scroll_lines = g.scroll_lines.saturating_sub(MOUSE_SCROLL_LINES);
                            }
                            MouseEventKind::ScrollDown => {
                                g.scroll_lines =
                                    (g.scroll_lines + MOUSE_SCROLL_LINES).min(max_scroll);
                                if g.scroll_lines >= max_scroll {
                                    g.transcript_follow_tail = true;
                                }
                            }
                            MouseEventKind::Down(MouseButton::Left) => {
                                // Inner content starts below top border (y+1).
                                let inner_top = tr.y.saturating_add(1);
                                if m.row >= inner_top {
                                    let row_in_area = (m.row - inner_top) as usize;
                                    if row_in_area < th {
                                        let gline = g.scroll_lines + row_in_area;
                                        let picked = if gline < hits.len() {
                                            hits[gline].clone().zip(
                                                g.active_question
                                                    .as_ref()
                                                    .map(|q| q.question_id.clone()),
                                            )
                                        } else {
                                            None
                                        };
                                        if let Some((sel, qid)) = picked {
                                            drop(g);
                                            if let Some(ref tx) = question_answer_tx {
                                                let _ = tx.send((qid, sel));
                                            } else {
                                                let _ = cmd_tx.send(TuiCmd::QuestionAnswer(sel));
                                            }
                                            continue;
                                        }
                                    }
                                }
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
                            let list_scroll = g
                                .slash_menu_index
                                .saturating_sub(n_show.saturating_sub(1))
                                .min(max_scroll);
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
                Event::Key(key) => {
                    if g.command_palette_open {
                        match (key.code, key.modifiers) {
                            (KeyCode::Esc, _) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                                g.command_palette_open = false;
                                g.command_palette_query.clear();
                                g.slash_menu_index = 0;
                            }
                            (KeyCode::Up, _) => {
                                g.slash_menu_index = g.slash_menu_index.saturating_sub(1);
                            }
                            (KeyCode::Down, _) => {
                                let filtered = filter_command_palette(&g.command_palette_query);
                                if !filtered.is_empty() {
                                    g.slash_menu_index = (g.slash_menu_index + 1) % filtered.len();
                                }
                            }
                            (KeyCode::Enter, _) => {
                                let filtered = filter_command_palette(&g.command_palette_query);
                                if let Some(cmd) = filtered
                                    .get(g.slash_menu_index.min(filtered.len().saturating_sub(1)))
                                {
                                    g.input_buffer = (*cmd).to_string();
                                    g.cursor_char_idx = g.input_buffer.chars().count();
                                }
                                g.command_palette_open = false;
                                g.command_palette_query.clear();
                                g.slash_menu_index = 0;
                            }
                            (KeyCode::Backspace, _) => {
                                g.command_palette_query.pop();
                                let filtered = filter_command_palette(&g.command_palette_query);
                                if filtered.is_empty() {
                                    g.slash_menu_index = 0;
                                } else {
                                    g.slash_menu_index =
                                        g.slash_menu_index.min(filtered.len().saturating_sub(1));
                                }
                            }
                            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                                g.command_palette_query.push(c);
                                let filtered = filter_command_palette(&g.command_palette_query);
                                if filtered.is_empty() {
                                    g.slash_menu_index = 0;
                                } else {
                                    g.slash_menu_index =
                                        g.slash_menu_index.min(filtered.len().saturating_sub(1));
                                }
                            }
                            _ => {}
                        }
                        continue;
                    }

                    match (key.code, key.modifiers) {
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
                            g.transcript_follow_tail = true;
                        }
                        (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                            g.command_palette_open = true;
                            g.command_palette_query.clear();
                            g.slash_menu_index = 0;
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
                            let active_approval = g.active_approval.clone();
                            let active_q = g.active_question.clone();
                            if let Some(req) = active_approval {
                                let verdict = match line.trim().to_ascii_lowercase().as_str() {
                                    "y" | "yes" => Some(true),
                                    "n" | "no" => Some(false),
                                    _ => None,
                                };
                                if let Some(approved) = verdict {
                                    let call_id = req.call_id.clone();
                                    drop(g);
                                    if let Some(ref tx) = approval_answer_tx {
                                        let _ = tx.send((call_id, approved));
                                    } else {
                                        let _ = cmd_tx.send(TuiCmd::CancelTurn);
                                    }
                                    continue;
                                }
                                g.blocks.push(DisplayBlock::System(
                                    "Invalid approval answer: use y/yes or n/no.".into(),
                                ));
                                continue;
                            }
                            if let Some(ref q) = active_q {
                                let t = line.trim();
                                // `/auto-answer` must go through the side channel: `run_turn` is often
                                // blocked on this question, so `cmd_rx` is not polled for Submit.
                                if t == "/auto-answer" {
                                    let qid = q.question_id.clone();
                                    drop(g);
                                    if let Some(ref tx) = question_answer_tx {
                                        let _ = tx.send((qid, QuestionSelection::Suggested));
                                    } else {
                                        let _ = cmd_tx.send(TuiCmd::QuestionAnswer(
                                            QuestionSelection::Suggested,
                                        ));
                                    }
                                    continue;
                                }
                                if t.starts_with('/') {
                                    drop(g);
                                    let _ = cmd_tx.send(TuiCmd::Submit(line));
                                    continue;
                                }
                                if let Some(sel) = parse_tui_question_answer(&line, q) {
                                    let qid = q.question_id.clone();
                                    drop(g);
                                    if let Some(ref tx) = question_answer_tx {
                                        let _ = tx.send((qid, sel));
                                    } else {
                                        let _ = cmd_tx.send(TuiCmd::QuestionAnswer(sel));
                                    }
                                    continue;
                                }
                                g.blocks.push(DisplayBlock::System(
                                    "Invalid answer: use Enter/0 for suggested, 1–n for an option, or custom text."
                                        .into(),
                                ));
                                continue;
                            }
                            drop(g);
                            let _ = cmd_tx.send(TuiCmd::Submit(line));
                        }
                        (KeyCode::Char('a'), KeyModifiers::CONTROL) | (KeyCode::Home, _) => {
                            g.cursor_char_idx = 0;
                        }
                        (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
                            g.cursor_char_idx = g.input_buffer.chars().count();
                        }
                        (KeyCode::End, _) => {
                            if !g.input_buffer.is_empty() {
                                g.cursor_char_idx = g.input_buffer.chars().count();
                            } else {
                                let sz = terminal.size().ok();
                                if let Some(sz) = sz {
                                    let area = Rect::new(0, 0, sz.width, sz.height);
                                    let (main_area, _) = layout_with_sidebar(area);
                                    let sh = slash_panel_height(
                                        filter_slash_commands(&g.input_buffer).len(),
                                    );
                                    let (tr, _, _, _) = layout_chunks(main_area, sh);
                                    let total =
                                        transcript_lines(&g, tr.width.saturating_sub(2)).len();
                                    let th = tr.height.saturating_sub(2) as usize;
                                    let max_scroll = total.saturating_sub(th);
                                    g.transcript_follow_tail = true;
                                    g.scroll_lines = max_scroll;
                                }
                            }
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
                                g.transcript_follow_tail = false;
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
                                    let (main_area, _) = layout_with_sidebar(area);
                                    let sh = slash_panel_height(
                                        filter_slash_commands(&g.input_buffer).len(),
                                    );
                                    let (tr, _, _, _) = layout_chunks(main_area, sh);
                                    let lines = transcript_lines(&g, tr.width.saturating_sub(2));
                                    let total = lines.len();
                                    let th = tr.height.saturating_sub(2) as usize;
                                    let max_scroll = total.saturating_sub(th);
                                    g.scroll_lines = (g.scroll_lines + 1).min(max_scroll);
                                    if g.scroll_lines >= max_scroll {
                                        g.transcript_follow_tail = true;
                                    }
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
                    }
                }
                _ => {}
            }
        }
    }

    restore_terminal();
    let _ = execute!(stdout(), MoveToColumn(0));
    Ok(())
}
