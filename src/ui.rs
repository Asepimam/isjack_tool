//! Ratatui rendering — one draw function per tab, shared helpers.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
};
use crate::app::{ActiveTab, App, Focus, InputBuffer, IsoMode, KeyOp, SimMode};

const TAB_TITLES: [&str; 6] = [
    " F1 JSON ", " F2 ISO 8583 ", " F3 TLV/EMV ", " F4 Key Mgmt ", " F5 Simulator ", " F6 Settlement ",
];

// ─── Entry point ─────────────────────────────────────────────────────────────

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area(); // ratatui 0.29+  (.size() is deprecated)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tab bar
            Constraint::Min(0),    // content
            Constraint::Length(1), // status bar
        ])
        .split(area);

    draw_tabs(f, app, chunks[0]);
    draw_content(f, app, chunks[1]);
    draw_status(f, app, chunks[2]);
}

// ─── Tab bar ─────────────────────────────────────────────────────────────────

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let selected = app.active_tab.index();
    let titles: Vec<Line> = TAB_TITLES
        .iter()
        .enumerate()
        .map(|(i, &t)| {
            let style = if i == selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Line::from(Span::styled(t, style))
        })
        .collect();

    let tabs = Tabs::new(titles)
        .select(selected)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(" ⬡ IsJack Toolkit v0.1 "))
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    f.render_widget(tabs, area);
}

// ─── Content dispatcher ───────────────────────────────────────────────────────

fn draw_content(f: &mut Frame, app: &mut App, area: Rect) {
    match app.active_tab {
        ActiveTab::Json       => draw_split_pane(f, app, area, PaneConfig::json()),
        ActiveTab::Iso8583    => draw_split_pane(f, app, area, PaneConfig::iso(app.iso_mode)),
        ActiveTab::Tlv        => draw_split_pane(f, app, area, PaneConfig::tlv()),
        ActiveTab::KeyMgmt    => draw_key_mgmt(f, app, area),
        ActiveTab::Simulator  => draw_simulator(f, app, area),
        ActiveTab::Settlement => draw_split_pane(f, app, area, PaneConfig::settlement()),
    }
}

// ─── Split-pane layout (input left | output right) ───────────────────────────

struct PaneConfig {
    input_title:  String,
    output_title: &'static str,
    input_pct:    u16,
}

impl PaneConfig {
    fn json() -> Self {
        Self {
            input_title:  " Input — edit JSON │ 's'=toggle mode │ 'n'=next sample ".to_string(),
            output_title: " Output — Space=beautify/minify │ ↑↓/PgUp/PgDn=scroll ",
            input_pct:    40,
        }
    }
    fn iso(mode: IsoMode) -> Self {
        let m = match mode { IsoMode::Hex => "HEX", IsoMode::Raw => "RAW/ASCII" };
        Self {
            input_title:  format!(" Input — ISO 8583 ({}) │ 'd'=toggle mode │ 'n'=next sample ", m),
            output_title: " Output — Space=decode │ ↑↓/PgUp/PgDn=scroll ",
            input_pct:    40,
        }
    }
    fn tlv() -> Self {
        Self {
            input_title:  " Input — TLV/EMV hex │ 'n'=next sample ".to_string(),
            output_title: " Output — Space=decode │ ↑↓/PgUp/PgDn=scroll ",
            input_pct:    35,
        }
    }
    fn settlement() -> Self {
        Self {
            input_title:  " Input — CSV transactions │ 'r'=reload sample ".to_string(),
            output_title: " Report — Space=parse │ ↑↓/PgUp/PgDn=scroll ",
            input_pct:    45,
        }
    }
}

fn draw_split_pane(f: &mut Frame, app: &mut App, area: Rect, cfg: PaneConfig) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(cfg.input_pct),
            Constraint::Percentage(100 - cfg.input_pct),
        ])
        .split(area);

    let input_focused  = app.focus == Focus::Input;
    let output_focused = app.focus == Focus::Output;

    // ── Input pane ──────────────────────────────────────────────────────────
    let in_block = Block::default()
        .borders(Borders::ALL)
        .title(if input_focused {
            format!("▶{}", cfg.input_title)
        } else {
            cfg.input_title.clone()
        })
        .border_style(focus_border(input_focused));

    let in_inner = in_block.inner(chunks[0]);
    f.render_widget(in_block, chunks[0]);

    // Determine which input buffer belongs to the current tab
    let visible_rows = in_inner.height;

    // We need the buf immutably for rendering — but sync_scroll needs &mut.
    // Solution: call sync_scroll first (mutably), then render immutably.
    sync_active_input(app, visible_rows);

    let (lines, cur_row, cur_col, scroll) = borrow_input_view(app);
    render_input_pane(f, in_inner, lines, cur_row, cur_col, scroll, input_focused);

    // ── Output pane ─────────────────────────────────────────────────────────
    let out_block = Block::default()
        .borders(Borders::ALL)
        .title(if output_focused {
            format!("▶{}", cfg.output_title)
        } else {
            cfg.output_title.to_string()
        })
        .border_style(focus_border(output_focused));

    let out_inner = out_block.inner(chunks[1]);
    f.render_widget(out_block, chunks[1]);

    let (content, scroll) = borrow_output_view(app);
    render_output_pane(f, out_inner, content, scroll);
}

/// Call `sync_scroll` on the active tab's input buffer using actual viewport height.
fn sync_active_input(app: &mut App, visible_rows: u16) {
    match app.active_tab {
        ActiveTab::Json       => app.json_input.sync_scroll(visible_rows),
        ActiveTab::Iso8583    => app.iso_input.sync_scroll(visible_rows),
        ActiveTab::Tlv        => app.tlv_input.sync_scroll(visible_rows),
        ActiveTab::Settlement => app.settle_input.sync_scroll(visible_rows),
        ActiveTab::KeyMgmt    => {
            let f = app.key_focus_field.min(2) as usize;
            app.key_field[f].sync_scroll(visible_rows);
        }
        ActiveTab::Simulator  => app.sim_message.sync_scroll(visible_rows),
    }
}

/// Immutable view into the active input buffer: (lines, cursor_row, cursor_col, scroll)
fn borrow_input_view(app: &App) -> (&[String], usize, usize, u16) {
    let buf: &InputBuffer = match app.active_tab {
        ActiveTab::Json       => &app.json_input,
        ActiveTab::Iso8583    => &app.iso_input,
        ActiveTab::Tlv        => &app.tlv_input,
        ActiveTab::Settlement => &app.settle_input,
        ActiveTab::KeyMgmt    => &app.key_field[app.key_focus_field.min(2) as usize],
        ActiveTab::Simulator  => &app.sim_message,
    };
    (&buf.lines, buf.cursor_row, buf.cursor_col, buf.scroll)
}

/// Immutable view into the active output buffer: (content, scroll)
fn borrow_output_view(app: &App) -> (&str, u16) {
    let buf = match app.active_tab {
        ActiveTab::Json       => &app.json_output,
        ActiveTab::Iso8583    => &app.iso_output,
        ActiveTab::Tlv        => &app.tlv_output,
        ActiveTab::Settlement => &app.settle_output,
        ActiveTab::KeyMgmt    => &app.key_output,
        ActiveTab::Simulator  => &app.sim_output,
    };
    (&buf.content, buf.scroll)
}

/// Render the input lines into `area`, highlighting the cursor row/col when focused.
fn render_input_pane(
    f: &mut Frame,
    area: Rect,
    lines: &[String],
    cursor_row: usize,
    cursor_col: usize,
    scroll: u16,
    focused: bool,
) {
    let start = scroll as usize;
    let end   = (start + area.height as usize).min(lines.len());

    let rendered: Vec<Line> = lines[start..end]
        .iter()
        .enumerate()
        .map(|(rel, line)| {
            if focused && (start + rel) == cursor_row {
                cursor_line(line, cursor_col)
            } else {
                Line::from(Span::raw(line.as_str().to_owned()))
            }
        })
        .collect();

    f.render_widget(Paragraph::new(rendered), area);
}

/// Render output content with syntax colouring.
fn render_output_pane(f: &mut Frame, area: Rect, content: &str, scroll: u16) {
    let rendered: Vec<Line> = content
        .lines()
        .skip(scroll as usize)
        .take(area.height as usize)
        .map(|l| Line::from(colorize_line(l)))
        .collect();

    f.render_widget(Paragraph::new(rendered), area);
}

// ─── Key Management tab ──────────────────────────────────────────────────────

fn draw_key_mgmt(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // operation selector
            Constraint::Length(3), // field 0
            Constraint::Length(3), // field 1
            Constraint::Length(3), // field 2 (only some ops)
            Constraint::Min(0),    // output
        ])
        .split(area);

    // ── Operation selector ──────────────────────────────────────────────────
    let op_spans: Vec<Span> = KeyOp::all()
        .iter()
        .map(|&op| {
            let style = if op == app.key_op {
                Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Span::styled(format!(" {} ", op.label()), style)
        })
        .collect();

    let op_block = Block::default()
        .borders(Borders::ALL)
        .title(" Operation — 'o'=cycle │ 'n'=sample │ Space=run │ Tab=next field ");
    let op_inner = op_block.inner(chunks[0]);
    f.render_widget(op_block, chunks[0]);
    f.render_widget(Paragraph::new(Line::from(op_spans)), op_inner);

    // ── Input fields ────────────────────────────────────────────────────────
    let labels = app.key_op.field_labels();
    for (idx, (&rect, &label)) in chunks[1..4].iter().zip(labels.iter()).enumerate() {
        if label.is_empty() { continue; }

        let is_focused = app.focus == Focus::Input && app.key_focus_field == idx as u8;
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", label))
            .border_style(focus_border(is_focused));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        // sync_scroll + render for this specific field
        app.key_field[idx].sync_scroll(inner.height);
        let buf = &app.key_field[idx];
        let visible_h = inner.height as usize;
        let start     = buf.scroll as usize;
        let end       = (start + visible_h).min(buf.lines.len());

        let rendered: Vec<Line> = buf.lines[start..end]
            .iter()
            .enumerate()
            .map(|(rel, line)| {
                if is_focused && (start + rel) == buf.cursor_row {
                    cursor_line(line, buf.cursor_col)
                } else {
                    Line::from(Span::raw(line.as_str().to_owned()))
                }
            })
            .collect();
        f.render_widget(Paragraph::new(rendered), inner);
    }

    // ── Output ──────────────────────────────────────────────────────────────
    let out_focused = app.focus == Focus::Output;
    let out_block = Block::default()
        .borders(Borders::ALL)
        .title(if out_focused { "▶ Result — Tab=back to fields │ ↑↓/PgUp/PgDn=scroll" } else { " Result " })
        .border_style(focus_border(out_focused));
    let out_inner = out_block.inner(chunks[4]);
    f.render_widget(out_block, chunks[4]);

    let rendered: Vec<Line> = app.key_output.content
        .lines()
        .skip(app.key_output.scroll as usize)
        .take(out_inner.height as usize)
        .map(|l| Line::from(colorize_line(l)))
        .collect();
    f.render_widget(Paragraph::new(rendered), out_inner);
}

// ─── Simulator tab ───────────────────────────────────────────────────────────

fn draw_simulator(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // config bar
            Constraint::Length(4), // message input
            Constraint::Min(0),    // log
        ])
        .split(area);

    // ── Config bar ──────────────────────────────────────────────────────────
    let running  = app.sim_server.is_running();
    let mode_str = match app.sim_mode { SimMode::Server => "SERVER", SimMode::Client => "CLIENT" };

    let status_span = if running {
        Span::styled(
            format!(" ● RUNNING :{} ", app.sim_port),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(" ○ STOPPED ", Style::default().fg(Color::DarkGray))
    };

    let cfg_line = Line::from(vec![
        Span::styled(format!(" {} ", mode_str), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" │ "),
        status_span,
        Span::raw(" │ "),
        Span::styled(format!("framing:{} ", app.sim_framing), Style::default().fg(Color::Yellow)),
        Span::raw("  'm'=mode │ 'f'=framing │ Space=start/stop/send"),
    ]);

    let cfg_block = Block::default().borders(Borders::ALL).title(" Simulator ");
    let cfg_inner = cfg_block.inner(chunks[0]);
    f.render_widget(cfg_block, chunks[0]);
    f.render_widget(Paragraph::new(cfg_line), cfg_inner);

    // ── Message / port input ─────────────────────────────────────────────────
    let msg_title = match app.sim_mode {
        SimMode::Client => format!(" Send to {}:{} — hex message ", app.sim_host, app.sim_port),
        SimMode::Server => format!(" Port: {} — edit then Space to start/stop ", app.sim_port),
    };
    let msg_focused = app.focus == Focus::Input;
    let msg_block = Block::default()
        .borders(Borders::ALL)
        .title(msg_title)
        .border_style(focus_border(msg_focused));
    let msg_inner = msg_block.inner(chunks[1]);
    f.render_widget(msg_block, chunks[1]);

    app.sim_message.sync_scroll(msg_inner.height);
    let buf = &app.sim_message;
    let start = buf.scroll as usize;
    let end   = (start + msg_inner.height as usize).min(buf.lines.len());
    let rendered: Vec<Line> = buf.lines[start..end]
        .iter()
        .enumerate()
        .map(|(rel, line)| {
            if msg_focused && (start + rel) == buf.cursor_row {
                cursor_line(line, buf.cursor_col)
            } else {
                Line::from(Span::raw(line.as_str().to_owned()))
            }
        })
        .collect();
    f.render_widget(Paragraph::new(rendered), msg_inner);

    // ── Transaction log ──────────────────────────────────────────────────────
    let log_focused = app.focus == Focus::Output;
    let log_block = Block::default()
        .borders(Borders::ALL)
        .title(if log_focused {
            "▶ Transaction Log — ↑↓/PgUp/PgDn=scroll"
        } else {
            " Transaction Log "
        })
        .border_style(focus_border(log_focused));
    let log_inner = log_block.inner(chunks[2]);
    f.render_widget(log_block, chunks[2]);

    let rendered: Vec<Line> = app.sim_output.content
        .lines()
        .skip(app.sim_output.scroll as usize)
        .take(log_inner.height as usize)
        .map(|l| {
            let style = if l.contains("▼ RECV") {
                Style::default().fg(Color::Cyan)
            } else if l.contains("▲ SEND") {
                Style::default().fg(Color::Green)
            } else if l.contains("✗ ERR") {
                Style::default().fg(Color::Red)
            } else if l.contains("INFO") {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            };
            Line::from(Span::styled(l.to_owned(), style))
        })
        .collect();
    f.render_widget(Paragraph::new(rendered), log_inner);
}

// ─── Status bar ──────────────────────────────────────────────────────────────

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let style = if app.status_is_error {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {}", app.status),
            style,
        ))),
        area,
    );
}

// ─── Shared rendering helpers ─────────────────────────────────────────────────

/// Border style for a focused / unfocused pane.
fn focus_border(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

/// Build a `Line` for a cursor row: text before cursor | cursor char (highlighted) | text after.
/// Accepts `&str` — works whether the caller has a `String` or a `&str`.
pub fn cursor_line(line: &str, cursor_col: usize) -> Line<'static> {
    let chars: Vec<char> = line.chars().collect();
    let col = cursor_col.min(chars.len());

    let before: String = chars[..col].iter().collect();
    let at: String = if col < chars.len() {
        chars[col].to_string()
    } else {
        " ".to_string() // show cursor past end of line
    };
    let after: String = if col + 1 < chars.len() {
        chars[col + 1..].iter().collect()
    } else {
        String::new()
    };

    Line::from(vec![
        Span::raw(before),
        Span::styled(at, Style::default().fg(Color::Black).bg(Color::Yellow)),
        Span::raw(after),
    ])
}

/// Syntax-colour a single output line.
/// Returns `Span<'static>` so it can be placed into `Line` without lifetime issues.
pub fn colorize_line(line: &str) -> Span<'static> {
    let s = line.to_owned();

    // Box-drawing / section headers
    if s.starts_with('╔') || s.starts_with('╚') || s.starts_with('║') || s.starts_with("──") {
        return Span::styled(s, Style::default().fg(Color::Blue));
    }
    // Errors / warnings
    if s.contains("Error") || s.contains("error") || s.contains('✗') || s.contains('⚠') {
        return Span::styled(s, Style::default().fg(Color::Red));
    }
    // ISO 8583 field rows (start with spaces + F0xx/F1xx)
    if s.trim_start().starts_with('F') {
        let trimmed = s.trim_start();
        if trimmed.len() > 4 && trimmed[1..4].chars().all(|c| c.is_ascii_digit()) {
            return Span::styled(s, Style::default().fg(Color::Cyan));
        }
    }
    // MTI / bitmap lines
    if s.contains("MTI") || s.contains("Bitmap") || s.contains("bitmap") {
        return Span::styled(s, Style::default().fg(Color::Magenta));
    }
    // Financial totals
    if s.contains("NET SETTLEMENT") || s.contains("KCV") || s.contains("Cryptogram") {
        return Span::styled(s, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
    }
    // Status codes
    if s.contains("APPROVED") {
        return Span::styled(s, Style::default().fg(Color::Green));
    }
    if s.contains("DECLINED") || s.contains("REVERSED") {
        return Span::styled(s, Style::default().fg(Color::Red));
    }
    // TLV tag lines
    if s.trim_start().starts_with("┌─") || s.trim_start().starts_with("│") || s.trim_start().starts_with("└─") {
        return Span::styled(s, Style::default().fg(Color::Cyan));
    }

    Span::raw(s)
}
