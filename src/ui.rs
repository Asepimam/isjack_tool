use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs, Wrap},
};

use crate::app::{ActiveTab, App, Focus, IsoMode, JsonMode};

const TAB_TITLES: [&str; 2] = [" 󰘦 JSON Beautify/Minify ", " 󰙧 ISO 8583 Decoder "];
const COLOR_ACCENT: Color  = Color::Cyan;
const COLOR_ACTIVE: Color  = Color::Yellow;
const COLOR_BORDER: Color  = Color::DarkGray;
const COLOR_FOCUSED:Color  = Color::Cyan;
const COLOR_ERROR:  Color  = Color::Red;
const COLOR_OK:     Color  = Color::Green;
const COLOR_DIM:    Color  = Color::DarkGray;
const COLOR_BG:     Color  = Color::Reset;

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();

    // ── Root layout: header + body + status ──
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // tabs header
            Constraint::Min(10),    // main body
            Constraint::Length(1),  // status bar
        ])
        .split(area);

    draw_tabs(f, app, root[0]);
    draw_body(f, app, root[1]);
    draw_status(f, app, root[2]);
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let tab_idx = match app.active_tab {
        ActiveTab::Json   => 0,
        ActiveTab::Iso8583 => 1,
    };

    let titles: Vec<Line> = TAB_TITLES.iter().enumerate().map(|(i, t)| {
        if i == tab_idx {
            Line::from(Span::styled(*t, Style::default().fg(COLOR_ACTIVE).add_modifier(Modifier::BOLD)))
        } else {
            Line::from(Span::styled(*t, Style::default().fg(COLOR_DIM)))
        }
    }).collect();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .title(Span::styled("  ISJACK Tool  ", Style::default().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD)))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(COLOR_BORDER)),
        )
        .select(tab_idx)
        .highlight_style(Style::default().fg(COLOR_ACTIVE).add_modifier(Modifier::BOLD));

    f.render_widget(tabs, area);
}

fn draw_body(f: &mut Frame, app: &mut App, area: Rect) {
    // Split body into left (input) and right (output)
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(45),
            Constraint::Percentage(55),
        ])
        .split(area);

    let input_area  = body[0];
    let output_area = body[1];

    match app.active_tab {
        ActiveTab::Json    => draw_json_panes(f, app, input_area, output_area),
        ActiveTab::Iso8583 => draw_iso_panes(f, app, input_area, output_area),
    }
}

// ─────────────────────────────────────────────────────────── JSON ──

fn draw_json_panes(f: &mut Frame, app: &mut App, input_area: Rect, output_area: Rect) {
    let mode_label = match app.json_mode {
        JsonMode::Beautify => "Beautify",
        JsonMode::Minify   => "Minify  ",
    };

    let input_focused  = app.focus == Focus::Input;
    let output_focused = app.focus == Focus::Output;

    // ── Input pane ──
    let input_border_style = if input_focused {
        Style::default().fg(COLOR_FOCUSED)
    } else {
        Style::default().fg(COLOR_BORDER)
    };

    let input_title = if input_focused {
        format!(" INPUT [Mode: {} | F6: toggle] ", mode_label)
    } else {
        format!(" INPUT [Mode: {}] ", mode_label)
    };

    // Sync scroll
    app.json_input.sync_scroll(input_area.height);

    // Build text lines with cursor highlight
    let lines = render_input_lines(&app.json_input, input_area, input_focused);

    let input_paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(Span::styled(input_title, Style::default().fg(if input_focused { COLOR_ACTIVE } else { COLOR_BORDER })))
                .borders(Borders::ALL)
                .border_style(input_border_style),
        )
        .scroll((app.json_input.scroll, 0));

    f.render_widget(input_paragraph, input_area);

    // ── Output pane ──
    let output_border_style = if output_focused {
        Style::default().fg(COLOR_FOCUSED)
    } else {
        Style::default().fg(COLOR_BORDER)
    };

    let out_lines: usize = app.json_output.content.lines().count();
    let output_paragraph = Paragraph::new(app.json_output.content.as_str())
        .block(
            Block::default()
                .title(Span::styled(
                    format!(" OUTPUT ({} lines) ", out_lines),
                    Style::default().fg(if output_focused { COLOR_ACTIVE } else { COLOR_BORDER }),
                ))
                .borders(Borders::ALL)
                .border_style(output_border_style),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.json_output.scroll, 0));

    f.render_widget(output_paragraph, output_area);

    // ── Help overlay (bottom of input when focused) ──
    if input_focused {
        draw_help_hint(f, input_area, "F5:Process  F6:Mode  Ctrl+L:Clear  Tab:→Output");
    }
    if output_focused {
        draw_help_hint(f, output_area, "↑↓/PgUp/PgDn:Scroll  g:Top  G:Bottom  Tab:→Input");
    }
}

// ──────────────────────────────────────────────────────── ISO 8583 ──

fn draw_iso_panes(f: &mut Frame, app: &mut App, input_area: Rect, output_area: Rect) {
    let input_focused  = app.focus == Focus::Input;
    let output_focused = app.focus == Focus::Output;

    // ── Input pane ──
    let input_border_style = if input_focused {
        Style::default().fg(COLOR_FOCUSED)
    } else {
        Style::default().fg(COLOR_BORDER)
    };

    app.iso_input.sync_scroll(input_area.height);

    let lines = render_input_lines(&app.iso_input, input_area, input_focused);

    let input_paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(Span::styled(
                    if input_focused { " HEX INPUT [ASCII encoding] " } else { " HEX INPUT " },
                    Style::default().fg(if input_focused { COLOR_ACTIVE } else { COLOR_BORDER }),
                ))
                .borders(Borders::ALL)
                .border_style(input_border_style),
        )
        .scroll((app.iso_input.scroll, 0));

    f.render_widget(input_paragraph, input_area);

    // ── Output pane ──
    let output_border_style = if output_focused {
        Style::default().fg(COLOR_FOCUSED)
    } else {
        Style::default().fg(COLOR_BORDER)
    };

    let out_lines: usize = app.iso_output.content.lines().count();
    let output_paragraph = Paragraph::new(app.iso_output.content.as_str())
        .block(
            Block::default()
                .title(Span::styled(
                    format!(" DECODE RESULT ({} lines) ", out_lines),
                    Style::default().fg(if output_focused { COLOR_ACTIVE } else { COLOR_BORDER }),
                ))
                .borders(Borders::ALL)
                .border_style(output_border_style),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.iso_output.scroll, 0));

    f.render_widget(output_paragraph, output_area);

    if input_focused {
        let iso_hint = match app.iso_mode {
            IsoMode::Hex => "F5:Decode  F6:→RAW mode  Ctrl+L:Clear  Tab:→Output",
            IsoMode::Raw => "F5:Decode  F6:→HEX mode  Ctrl+L:Clear  Tab:→Output",
        };
        draw_help_hint(f, input_area, iso_hint);
    }
    if output_focused {
        draw_help_hint(f, output_area, "↑↓/PgUp/PgDn:Scroll  g:Top  G:Bottom  Tab:→Input");
    }
}

// ─────────────────────────────────────────────────────── Helpers ──

/// Render input buffer lines with cursor highlight
fn render_input_lines<'a>(buf: &'a crate::app::InputBuffer, _area: Rect, focused: bool) -> Vec<Line<'a>> {
    buf.lines.iter().enumerate().map(|(row, line)| {
        if focused && row == buf.cursor_row {
            // Highlight cursor position
            let chars: Vec<char> = line.chars().collect();
            let col = buf.cursor_col.min(chars.len());

            let before: String = chars[..col].iter().collect();
            let cursor_char: String = if col < chars.len() {
                chars[col].to_string()
            } else {
                " ".to_string()
            };
            let after: String = if col < chars.len() {
                chars[col + 1..].iter().collect()
            } else {
                String::new()
            };

            Line::from(vec![
                Span::raw(before),
                Span::styled(cursor_char, Style::default().bg(COLOR_ACCENT).fg(Color::Black)),
                Span::raw(after),
            ])
        } else {
            Line::from(Span::raw(line.as_str()))
        }
    }).collect()
}

/// Draw a small hint bar at the bottom of an area (inside the border)
fn draw_help_hint(f: &mut Frame, area: Rect, hint: &str) {
    if area.height < 4 { return; }
    let hint_area = Rect {
        x: area.x + 1,
        y: area.y + area.height - 2,
        width: area.width.saturating_sub(2),
        height: 1,
    };
    let hint_p = Paragraph::new(Span::styled(
        format!(" {} ", hint),
        Style::default().fg(Color::Black).bg(COLOR_DIM),
    ));
    f.render_widget(hint_p, hint_area);
}

/// Draw status bar
fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let style = if app.status_is_error {
        Style::default().fg(COLOR_ERROR).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_OK)
    };

    // Left side: status message
    // Right side: tab/position info
    let right_info = {
        let (row, col) = match app.active_tab {
            ActiveTab::Json    => (app.json_input.cursor_row, app.json_input.cursor_col),
            ActiveTab::Iso8583 => (app.iso_input.cursor_row, app.iso_input.cursor_col),
        };
        format!(" Ln:{} Col:{} | F1:JSON  F2:ISO8583  Ctrl+Q:Quit ", row + 1, col + 1)
    };

    let max_status_width = area.width.saturating_sub(right_info.len() as u16 + 2) as usize;
    let status_text = if app.status.chars().count() > max_status_width {
        let truncated: String = app
            .status
            .chars()
            .take(max_status_width.saturating_sub(1))
            .collect();
        format!(" {}…", truncated)
    } else {
        format!(" {}", app.status)
    };

    let status_line = Line::from(vec![
        Span::styled(status_text, style),
        Span::styled(right_info, Style::default().fg(COLOR_DIM)),
    ]);

    let status_p = Paragraph::new(status_line)
        .style(Style::default().bg(Color::Reset));

    f.render_widget(status_p, area);
}
