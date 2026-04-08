//! Ratatui rendering — ISJack-Tools
//! Input and output panes both use visual-row word-wrap + scroll.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
};
use crate::app::{ActiveTab, App, Focus, InputBuffer, KeyOp, SimMode};

const TAB_TITLES: [&str; 6] = [
    " F1 JSON ", " F2 ISO 8583 ", " F3 TLV/EMV ", " F4 Key Mgmt ", " F5 Simulator ", " F6 Settlement ",
];

// ─── Entry point ──────────────────────────────────────────────────────────────

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    draw_tabs(f, app, chunks[0]);
    draw_content(f, app, chunks[1]);
    draw_status(f, app, chunks[2]);
}

// ─── Tab bar ─────────────────────────────────────────────────────────────────

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let selected = app.active_tab.index();
    let titles: Vec<Line> = TAB_TITLES.iter().enumerate().map(|(i, &t)| {
        let style = if i == selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        Line::from(Span::styled(t, style))
    }).collect();

    f.render_widget(
        Tabs::new(titles)
            .select(selected)
            .block(Block::default().borders(Borders::ALL).title(" ⬡ ISJack-Tools v1.1— Payment Gateway Toolkit "))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        area,
    );
}

fn draw_content(f: &mut Frame, app: &mut App, area: Rect) {
    let wrap_status = if active_input_buf(app).wrap { "WRAP" } else { "NOWRAP" };
    match app.active_tab {
        ActiveTab::Json       => draw_split(f, app, area, 40,
            &format!(" Input — {} │ 's' mode │ 'n' sample │ Ctrl+A select all │ Ctrl+Shift+C/X/V copy/cut/paste │ Ctrl+W toggle wrap", wrap_status),
            " Output — Space=run │ ↑↓ PgUp PgDn scroll | Ctrl + C to Copy"),
        ActiveTab::Iso8583    => draw_split(f, app, area, 35,
            &format!(" Input — {} │ 'd' mode │ 'n' sample │ paste ISO message │ Ctrl+W toggle wrap", wrap_status),
            " Output — Space=decode │ ↑↓ PgUp PgDn scroll "),
        ActiveTab::Tlv        => draw_split(f, app, area, 35,
            &format!(" Input — {} │ TLV/EMV hex │ 'n' sample │ Ctrl+W toggle wrap", wrap_status),
            " Output — Space=decode │ ↑↓ PgUp PgDn scroll "),
        ActiveTab::KeyMgmt    => draw_key_mgmt(f, app, area),
        ActiveTab::Simulator  => draw_simulator(f, app, area),
        ActiveTab::Settlement => draw_split(f, app, area, 45,
            &format!(" Input — {} │ CSV │ 'r' reload sample │ Ctrl+W toggle wrap", wrap_status),
            " Report — Space=parse │ ↑↓ PgUp PgDn scroll "),
    }
}

// ─── Generic split pane ───────────────────────────────────────────────────────

fn draw_split(f: &mut Frame, app: &mut App, area: Rect, in_pct: u16,
              in_title: &str, out_title: &'static str)
{
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(in_pct), Constraint::Percentage(100 - in_pct)])
        .split(area);

    let in_focused  = app.focus == Focus::Input;
    let out_focused = app.focus == Focus::Output;

    // ── INPUT ────────────────────────────────────────────────────────────────
    let input_title = build_input_title(app, in_focused, in_title, chunks[0]);
    let in_block = Block::default()
        .borders(Borders::ALL)
        .title(input_title)
        .border_style(border_style(in_focused));
    let in_inner = in_block.inner(chunks[0]);
    f.render_widget(in_block, chunks[0]);

    sync_and_render_input(f, app, in_inner, in_focused);

    // ── OUTPUT ───────────────────────────────────────────────────────────────
    // FIX 2: build_output_title sekarang menerima &mut App
    let output_title = build_output_title(app, out_focused, out_title, chunks[1]);
    let out_block = Block::default()
        .borders(Borders::ALL)
        .title(output_title)
        .border_style(border_style(out_focused));
    let out_inner = out_block.inner(chunks[1]);
    f.render_widget(out_block, chunks[1]);

    render_output(f, app, out_inner);
}

fn build_input_title(app: &App, focused: bool, base: &str, area: Rect) -> String {
    let buf = active_input_buf(app);
    let line_count = buf.lines.len();
    let vis_h = (area.height as usize).max(1);
    let vis_w = area.width as usize;

    let mut title = if focused { format!("▶ {}", base) } else { base.to_string() };

    if buf.wrap {
        if line_count > vis_h {
            if buf.scroll > 0 { title.push_str(" ↑"); }
            if (buf.scroll as usize) + vis_h < line_count { title.push_str(" ↓"); }
        }
    } else {
        let longest = buf.lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        if longest > vis_w {
            if buf.scroll_h > 0 { title.push_str(" ←"); }
            if (buf.scroll_h as usize) + vis_w < longest { title.push_str(" →"); }
        }
    }

    title
}

// FIX 2: ubah &App → &mut App agar bisa memanggil current_output() yang butuh &mut self
fn build_output_title(app: &mut App, focused: bool, base: &str, area: Rect) -> String {
    let content = match app.active_tab {
        ActiveTab::Json       => &app.json_output.content,
        ActiveTab::Iso8583    => &app.iso_output.content,
        ActiveTab::Tlv        => &app.tlv_output.content,
        ActiveTab::Settlement => &app.settle_output.content,
        ActiveTab::KeyMgmt    => &app.key_output.content,
        ActiveTab::Simulator  => &app.sim_output.content,
    };

    let line_count = content.lines().count();
    let vis_h = (area.height as usize).max(1);
    let mut title = if focused { format!("▶ {}", base) } else { base.to_string() };

    if line_count > vis_h {
        if app.current_output().scroll > 0 { title.push_str(" ↑"); }
        if (app.current_output().scroll as usize) + vis_h < line_count { title.push_str(" ↓"); }
    }

    title
}

/// Sync scroll for the active input buffer using the actual viewport height,
/// then render the wrapped content.
fn sync_and_render_input(f: &mut Frame, app: &mut App, area: Rect, focused: bool) {
    let width = area.width.max(4) as usize;
    let height = area.height as usize;

    let buf = active_input_buf(app);
    let do_wrap = buf.wrap;

    if do_wrap {
        // ── WRAP MODE ──
        let (logical_lines, cursor_row, cursor_col, sel_anchor) = {
            let buf = active_input_buf(app);
            (buf.lines.clone(), buf.cursor_row, buf.cursor_col, buf.sel_anchor)
        };

        let visual = build_visual_rows(&logical_lines, width);

        let cursor_visual = visual.iter().position(|vr| {
            vr.logical_row == cursor_row
                && cursor_col >= vr.char_start
                && (cursor_col < vr.char_start + vr.char_len || vr.is_last_of_logical)
        }).unwrap_or(0);

        {
            let buf = active_input_buf_mut(app);
            let vis = height.max(1);
            let scroll = buf.scroll as usize;
            let new_scroll = if cursor_visual < scroll {
                cursor_visual as u16
            } else if cursor_visual >= scroll + vis {
                (cursor_visual - vis + 1) as u16
            } else {
                buf.scroll
            };
            buf.scroll = new_scroll;
        }

        let scroll = active_input_buf(app).scroll as usize;

        let rendered: Vec<Line> = visual.iter()
            .skip(scroll)
            .take(height)
            .map(|vr| {
                let line = &logical_lines[vr.logical_row];
                let chars: Vec<char> = line.chars().collect();
                let seg_chars = &chars[vr.char_start..(vr.char_start + vr.char_len).min(chars.len())];
                let seg: String = seg_chars.iter().collect();

                if focused && vr.logical_row == cursor_row {
                    let local_col = cursor_col.saturating_sub(vr.char_start);
                    if cursor_col >= vr.char_start && (cursor_col < vr.char_start + vr.char_len || vr.is_last_of_logical) {
                        make_cursor_line(&seg, local_col, vr.is_continuation)
                    } else if vr.is_continuation {
                        Line::from(vec![
                            Span::styled("↩ ", Style::default().fg(Color::DarkGray)),
                            Span::raw(seg),
                        ])
                    } else {
                        Line::from(Span::styled(seg, Style::default().fg(Color::White)))
                    }
                } else if vr.is_continuation {
                    Line::from(vec![
                        Span::styled("↩ ", Style::default().fg(Color::DarkGray)),
                        colorize_input_line(&seg),
                    ])
                } else {
                    Line::from(colorize_input_line(&seg))
                }
            })
            .collect();

        f.render_widget(Paragraph::new(rendered), area);

        let _ = sel_anchor;
    } else {
        // ── NO-WRAP MODE ──
        let (logical_lines, scroll_v, scroll_h) = {
            let buf = active_input_buf(app);
            (buf.lines.clone(), buf.scroll, buf.scroll_h)
        };

        let text = logical_lines.join("\n");
        let paragraph = Paragraph::new(text)
            .scroll((scroll_v, scroll_h));

        f.render_widget(paragraph, area);
    }
}

fn active_input_buf(app: &App) -> &InputBuffer {
    match app.active_tab {
        ActiveTab::Json       => &app.json_input,
        ActiveTab::Iso8583    => &app.iso_input,
        ActiveTab::Tlv        => &app.tlv_input,
        ActiveTab::Settlement => &app.settle_input,
        ActiveTab::KeyMgmt    => &app.key_field[app.key_focus_field.min(2) as usize],
        ActiveTab::Simulator  => &app.sim_message,
    }
}

fn active_input_buf_mut(app: &mut App) -> &mut InputBuffer {
    match app.active_tab {
        ActiveTab::Json       => &mut app.json_input,
        ActiveTab::Iso8583    => &mut app.iso_input,
        ActiveTab::Tlv        => &mut app.tlv_input,
        ActiveTab::Settlement => &mut app.settle_input,
        ActiveTab::KeyMgmt    => {
            let f = app.key_focus_field.min(2) as usize;
            &mut app.key_field[f]
        }
        ActiveTab::Simulator  => &mut app.sim_message,
    }
}

/// Render the active output buffer with word-wrap and syntax colouring.
fn render_output(f: &mut Frame, app: &mut App, area: Rect) {
    let width  = area.width.max(4) as usize;
    let height = area.height as usize;

    let (content, scroll) = match app.active_tab {
        ActiveTab::Json       => (&app.json_output.content,   app.json_output.scroll),
        ActiveTab::Iso8583    => (&app.iso_output.content,    app.iso_output.scroll),
        ActiveTab::Tlv        => (&app.tlv_output.content,    app.tlv_output.scroll),
        ActiveTab::Settlement => (&app.settle_output.content, app.settle_output.scroll),
        ActiveTab::KeyMgmt    => (&app.key_output.content,    app.key_output.scroll),
        ActiveTab::Simulator  => (&app.sim_output.content,    app.sim_output.scroll),
    };

    let visual = wrap_text_lines(content, width);
    let rendered: Vec<Line> = visual.iter()
        .skip(scroll as usize)
        .take(height)
        .map(|(text, is_cont)| {
            if *is_cont {
                Line::from(vec![
                    Span::styled("  ", Style::default().fg(Color::DarkGray)),
                    colorize_output_line(text),
                ])
            } else {
                Line::from(colorize_output_line(text))
            }
        })
        .collect();

    f.render_widget(Paragraph::new(rendered), area);
}

// ─── Visual row construction ──────────────────────────────────────────────────

struct VisualRow {
    logical_row:        usize,
    char_start:         usize,
    char_len:           usize,
    is_continuation:    bool,
    is_last_of_logical: bool,
}

fn build_visual_rows(lines: &[String], width: usize) -> Vec<VisualRow> {
    let w = width.max(1);
    let mut result = Vec::new();
    for (row_idx, line) in lines.iter().enumerate() {
        let total_chars = line.chars().count();
        if total_chars == 0 {
            result.push(VisualRow {
                logical_row: row_idx, char_start: 0, char_len: 0,
                is_continuation: false, is_last_of_logical: true,
            });
            continue;
        }
        let mut start = 0;
        let mut first = true;
        while start < total_chars {
            let end = (start + w).min(total_chars);
            let is_last = end == total_chars;
            result.push(VisualRow {
                logical_row: row_idx,
                char_start: start,
                char_len: end - start,
                is_continuation: !first,
                is_last_of_logical: is_last,
            });
            start = end;
            first = false;
        }
    }
    result
}

fn wrap_text_lines(content: &str, width: usize) -> Vec<(String, bool)> {
    let w = width.max(1);
    let mut result = Vec::new();
    for line in content.lines() {
        if line.is_empty() {
            result.push((String::new(), false));
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let mut start = 0;
        let mut first = true;
        while start < chars.len() {
            let end = (start + w).min(chars.len());
            result.push((chars[start..end].iter().collect(), !first));
            start = end;
            first = false;
        }
    }
    result
}

// ─── Key Management tab ──────────────────────────────────────────────────────

fn draw_key_mgmt(f: &mut Frame, app: &mut App, area: Rect) {
    let active_fields = app.key_op.active_field_count() as usize;
    let mut constraints = vec![Constraint::Length(3)];
    for _ in 0..active_fields { constraints.push(Constraint::Length(3)); }
    constraints.push(Constraint::Min(0));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let op_spans: Vec<Span> = KeyOp::all().iter().map(|&op| {
        let style = if op == app.key_op {
            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        Span::styled(format!(" {} ", op.label()), style)
    }).collect();
    let op_block = Block::default().borders(Borders::ALL)
        .title(" Operation — 'o'=cycle │ 'n'=sample │ Space=run │ Tab=next field ");
    let op_inner = op_block.inner(chunks[0]);
    f.render_widget(op_block, chunks[0]);
    f.render_widget(Paragraph::new(Line::from(op_spans)), op_inner);

    let labels = app.key_op.field_labels();
    let mut chunk_idx = 1usize;
    for idx in 0..3usize {
        let label = labels[idx];
        if label.is_empty() { continue; }
        let rect = chunks[chunk_idx];
        chunk_idx += 1;
        let is_focused = app.focus == Focus::Input && app.key_focus_field == idx as u8;
        let block = Block::default().borders(Borders::ALL)
            .title(format!(" {} ", label))
            .border_style(border_style(is_focused));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let width  = inner.width.max(4) as usize;
        let height = inner.height as usize;
        let (lines, cur_row, cur_col, do_wrap) = {
            let buf = &app.key_field[idx];
            (buf.lines.clone(), buf.cursor_row, buf.cursor_col, buf.wrap)
        };

        if do_wrap {
            let visual = build_visual_rows(&lines, width);
            let cursor_visual = visual.iter().position(|vr| {
                vr.logical_row == cur_row && cur_col >= vr.char_start
                    && (cur_col < vr.char_start + vr.char_len || vr.is_last_of_logical)
            }).unwrap_or(0);
            {
                let buf = &mut app.key_field[idx];
                let scroll = buf.scroll as usize;
                if cursor_visual < scroll { buf.scroll = cursor_visual as u16; }
                else if cursor_visual >= scroll + height.max(1) { buf.scroll = (cursor_visual - height + 1) as u16; }
            }
            let scroll = app.key_field[idx].scroll as usize;
            let rendered: Vec<Line> = visual.iter().skip(scroll).take(height).map(|vr| {
                let line = &lines[vr.logical_row];
                let chars: Vec<char> = line.chars().collect();
                let seg: String = chars[vr.char_start..(vr.char_start+vr.char_len).min(chars.len())].iter().collect();
                if is_focused && vr.logical_row == cur_row {
                    let local = cur_col.saturating_sub(vr.char_start);
                    if cur_col >= vr.char_start && (cur_col < vr.char_start + vr.char_len || vr.is_last_of_logical) {
                        make_cursor_line(&seg, local, vr.is_continuation)
                    } else {
                        Line::from(Span::raw(seg))
                    }
                } else if vr.is_continuation {
                    Line::from(vec![Span::styled("↩ ", Style::default().fg(Color::DarkGray)), Span::raw(seg)])
                } else {
                    Line::from(Span::raw(seg))
                }
            }).collect();
            f.render_widget(Paragraph::new(rendered), inner);
        } else {
            let scroll_v = app.key_field[idx].scroll;
            let scroll_h = app.key_field[idx].scroll_h;
            let text = lines.join("\n");
            f.render_widget(Paragraph::new(text).scroll((scroll_v, scroll_h)), inner);
        }
    }

    let out_focused = app.focus == Focus::Output;
    let out_chunk = chunks[chunk_idx];
    let out_block = Block::default().borders(Borders::ALL)
        .title(if out_focused { "▶ Result — Tab=back │ ↑↓ PgUp PgDn scroll" } else { " Result " })
        .border_style(border_style(out_focused));
    let out_inner = out_block.inner(out_chunk);
    f.render_widget(out_block, out_chunk);
    let width = out_inner.width.max(4) as usize;
    let visual = wrap_text_lines(&app.key_output.content, width);
    let rendered: Vec<Line> = visual.iter()
        .skip(app.key_output.scroll as usize).take(out_inner.height as usize)
        .map(|(t, cont)| if *cont {
            Line::from(vec![Span::styled("  ", Style::default().fg(Color::DarkGray)), colorize_output_line(t)])
        } else { Line::from(colorize_output_line(t)) })
        .collect();
    f.render_widget(Paragraph::new(rendered), out_inner);
}

// ─── Simulator tab ───────────────────────────────────────────────────────────

fn draw_simulator(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(5), Constraint::Min(0)])
        .split(area);

    let running  = app.sim_server.is_running();
    let mode_str = match app.sim_mode { SimMode::Server => "SERVER", SimMode::Client => "CLIENT" };
    let status_span = if running {
        Span::styled(format!(" ● RUNNING :{} ", app.sim_port), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    } else {
        Span::styled(" ○ STOPPED ", Style::default().fg(Color::DarkGray))
    };
    let cfg_line = Line::from(vec![
        Span::styled(format!(" {} ", mode_str), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" │ "),
        status_span,
        Span::raw(" │ "),
        Span::styled(format!("framing:{} ", app.sim_framing), Style::default().fg(Color::Yellow)),
        Span::raw(" 'm'=mode │ 'f'=framing │ 'n'=sample(client) │ Space=start/stop/send"),
    ]);
    let cfg_block = Block::default().borders(Borders::ALL).title(" ISJack Simulator ");
    let cfg_inner = cfg_block.inner(chunks[0]);
    f.render_widget(cfg_block, chunks[0]);
    f.render_widget(Paragraph::new(cfg_line), cfg_inner);

    let msg_title = match app.sim_mode {
        SimMode::Client => format!(" → {}:{} — hex message (RAW or HEX-encoded) ", app.sim_host, app.sim_port),
        SimMode::Server => format!(" Port: {} — Space to start/stop server ", app.sim_port),
    };
    let msg_focused = app.focus == Focus::Input;

    // FIX 3: prefix dengan `_` karena belum digunakan di title string
    let _wrap_status = if app.sim_message.wrap { "WRAP" } else { "NOWRAP" };

    let msg_block = Block::default().borders(Borders::ALL)
        .title(format!("{} │ Ctrl+W toggle wrap", msg_title))
        .border_style(border_style(msg_focused));
    let msg_inner = msg_block.inner(chunks[1]);
    f.render_widget(msg_block, chunks[1]);

    let width  = msg_inner.width.max(4) as usize;
    let height = msg_inner.height as usize;
    let (lines, cur_row, cur_col, do_wrap) = {
        let b = &app.sim_message;
        (b.lines.clone(), b.cursor_row, b.cursor_col, b.wrap)
    };

    if do_wrap {
        let visual = build_visual_rows(&lines, width);
        let cur_vis = visual.iter().position(|vr| {
            vr.logical_row == cur_row && cur_col >= vr.char_start
                && (cur_col < vr.char_start + vr.char_len || vr.is_last_of_logical)
        }).unwrap_or(0);
        {
            let b = &mut app.sim_message;
            let sc = b.scroll as usize;
            if cur_vis < sc { b.scroll = cur_vis as u16; }
            else if cur_vis >= sc + height.max(1) { b.scroll = (cur_vis - height + 1) as u16; }
        }
        let scroll = app.sim_message.scroll as usize;
        let rendered: Vec<Line> = visual.iter().skip(scroll).take(height).map(|vr| {
            let lc: Vec<char> = lines[vr.logical_row].chars().collect();
            let seg: String = lc[vr.char_start..(vr.char_start+vr.char_len).min(lc.len())].iter().collect();
            if msg_focused && vr.logical_row == cur_row {
                let lc2 = cur_col.saturating_sub(vr.char_start);
                if cur_col >= vr.char_start && (cur_col < vr.char_start + vr.char_len || vr.is_last_of_logical) {
                    make_cursor_line(&seg, lc2, vr.is_continuation)
                } else if vr.is_continuation {
                    Line::from(vec![Span::styled("↩ ", Style::default().fg(Color::DarkGray)), Span::raw(seg)])
                } else { Line::from(Span::raw(seg)) }
            } else if vr.is_continuation {
                Line::from(vec![Span::styled("↩ ", Style::default().fg(Color::DarkGray)), Span::raw(seg)])
            } else { Line::from(Span::raw(seg)) }
        }).collect();
        f.render_widget(Paragraph::new(rendered), msg_inner);
    } else {
        let scroll_v = app.sim_message.scroll;
        let scroll_h = app.sim_message.scroll_h;
        let text = lines.join("\n");
        f.render_widget(Paragraph::new(text).scroll((scroll_v, scroll_h)), msg_inner);
    }

    let log_focused = app.focus == Focus::Output;
    let log_block = Block::default().borders(Borders::ALL)
        .title(if log_focused { "▶ Log — ↑↓ PgUp PgDn scroll" } else { " Transaction Log " })
        .border_style(border_style(log_focused));
    let log_inner = log_block.inner(chunks[2]);
    f.render_widget(log_block, chunks[2]);
    let lw = log_inner.width.max(4) as usize;
    let log_visual = wrap_text_lines(&app.sim_output.content, lw);
    let log_lines: Vec<Line> = log_visual.iter()
        .skip(app.sim_output.scroll as usize).take(log_inner.height as usize)
        .map(|(l, _)| {
            let style = if l.contains("▼ RECV") { Style::default().fg(Color::Cyan) }
                else if l.contains("▲ SEND") { Style::default().fg(Color::Green) }
                else if l.contains("✗ ERR")  { Style::default().fg(Color::Red) }
                else if l.contains("INFO")   { Style::default().fg(Color::DarkGray) }
                else { Style::default() };
            Line::from(Span::styled(l.clone(), style))
        }).collect();
    f.render_widget(Paragraph::new(log_lines), log_inner);
}

// ─── Status bar ──────────────────────────────────────────────────────────────

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let style = if app.status_is_error {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(format!(" {}", app.status), style))),
        area,
    );
}

// ─── Rendering helpers ────────────────────────────────────────────────────────

fn border_style(focused: bool) -> Style {
    if focused { Style::default().fg(Color::Yellow) }
    else       { Style::default().fg(Color::DarkGray) }
}

pub fn make_cursor_line(segment: &str, local_col: usize, is_cont: bool) -> Line<'static> {
    let chars: Vec<char> = segment.chars().collect();
    let col = local_col.min(chars.len());
    let prefix = if is_cont { "↩ " } else { "" };
    let before: String = chars[..col].iter().collect();
    let at: String = if col < chars.len() { chars[col].to_string() } else { " ".to_string() };
    let after:  String = if col + 1 < chars.len() { chars[col+1..].iter().collect() } else { String::new() };
    Line::from(vec![
        Span::styled(prefix, Style::default().fg(Color::DarkGray)),
        Span::raw(before),
        Span::styled(at, Style::default().fg(Color::Black).bg(Color::Yellow)),
        Span::raw(after),
    ])
}

fn colorize_input_line(line: &str) -> Span<'static> {
    let s = line.to_owned();
    if s.contains("\":") { return Span::styled(s, Style::default().fg(Color::White)); }
    if s.len() > 6 && s.chars().all(|c| c.is_ascii_hexdigit() || c.is_whitespace()) {
        return Span::styled(s, Style::default().fg(Color::Cyan));
    }
    Span::raw(s)
}

pub fn colorize_output_line(line: &str) -> Span<'static> {
    let s = line.to_owned();
    if s.starts_with('╔') || s.starts_with('╚') || s.starts_with('║') || s.starts_with("──") {
        return Span::styled(s, Style::default().fg(Color::Blue));
    }
    if s.contains("Error") || s.contains("error") || s.contains('✗') || s.contains('⚠') {
        return Span::styled(s, Style::default().fg(Color::Red));
    }
    let trimmed = s.trim_start();
    if trimmed.starts_with('F') && trimmed.len() > 4 && trimmed[1..4].chars().all(|c| c.is_ascii_digit()) {
        return Span::styled(s, Style::default().fg(Color::Cyan));
    }
    if s.contains("MTI") || s.contains("Bitmap") {
        return Span::styled(s, Style::default().fg(Color::Magenta));
    }
    if s.contains("NET SETTLEMENT") || s.contains("KCV   :") || s.contains("PIN   :") || s.contains("Cryptogram") {
        return Span::styled(s, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
    }
    if s.contains("APPROVED") || s.contains("Plaintext") {
        return Span::styled(s, Style::default().fg(Color::Green));
    }
    if s.contains("DECLINED") || s.contains("REVERSED") {
        return Span::styled(s, Style::default().fg(Color::Red));
    }
    if trimmed.starts_with("┌─") || trimmed.starts_with("│") || trimmed.starts_with("└─") {
        return Span::styled(s, Style::default().fg(Color::Cyan));
    }
    Span::raw(s)
}