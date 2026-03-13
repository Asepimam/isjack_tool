//! Application state — input/output buffers, tab state, all tool configs.

use std::sync::{Arc, Mutex};
use crate::simulator::{SimServer, SimState};

// ─── Tab identifiers ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActiveTab {
    Json,
    Iso8583,
    Tlv,
    KeyMgmt,
    Simulator,
    Settlement,
}

impl ActiveTab {
    pub fn index(self) -> usize {
        match self {
            ActiveTab::Json       => 0,
            ActiveTab::Iso8583    => 1,
            ActiveTab::Tlv        => 2,
            ActiveTab::KeyMgmt    => 3,
            ActiveTab::Simulator  => 4,
            ActiveTab::Settlement => 5,
        }
    }
}

// ─── Pane focus ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Focus { Input, Output }

// ─── Mode enums ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum JsonMode { Beautify, Minify }

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum IsoMode { Hex, Raw }

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SimMode { Server, Client }

// ─── Key Management operations ───────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum KeyOp {
    Kcv,
    TdesEncrypt,
    TdesDecrypt,
    PinBuild,
    PinDecrypt,
    XorHex,
    LuhnBin,
}

impl KeyOp {
    pub fn label(self) -> &'static str {
        match self {
            KeyOp::Kcv         => "KCV",
            KeyOp::TdesEncrypt => "3DES Enc",
            KeyOp::TdesDecrypt => "3DES Dec",
            KeyOp::PinBuild    => "PIN Build",
            KeyOp::PinDecrypt  => "PIN Decrypt",
            KeyOp::XorHex      => "XOR",
            KeyOp::LuhnBin     => "Luhn/BIN",
        }
    }

    pub fn all() -> &'static [KeyOp] {
        &[
            KeyOp::Kcv, KeyOp::TdesEncrypt, KeyOp::TdesDecrypt,
            KeyOp::PinBuild, KeyOp::PinDecrypt, KeyOp::XorHex, KeyOp::LuhnBin,
        ]
    }

    pub fn next(self) -> Self {
        let all = KeyOp::all();
        let pos = all.iter().position(|&k| k == self).unwrap_or(0);
        all[(pos + 1) % all.len()]
    }

    /// (label_field1, label_field2, label_field3)
    /// Empty string = field not used for this operation
    pub fn field_labels(self) -> [&'static str; 3] {
        match self {
            KeyOp::Kcv         => ["Key (hex 8/16/24 bytes)", "",                    ""],
            KeyOp::TdesEncrypt => ["Key (hex 16/24 bytes)",   "Data (hex)",          ""],
            KeyOp::TdesDecrypt => ["Key (hex 16/24 bytes)",   "Data (hex)",          ""],
            KeyOp::PinBuild    => ["PIN (4-12 digits)",       "PAN",                 ""],
            KeyOp::PinDecrypt  => ["ZPK (hex 16 bytes)",      "Enc PIN Block (hex)", "PAN"],
            KeyOp::XorHex      => ["Hex value A",             "Hex value B",         ""],
            KeyOp::LuhnBin     => ["PAN / Card Number",       "",                    ""],
        }
    }

    pub fn active_field_count(self) -> u8 {
        self.field_labels().iter().filter(|s| !s.is_empty()).count() as u8
    }
}

// ─── InputBuffer ─────────────────────────────────────────────────────────────
//
// Stores multi-line text with a character-indexed cursor.
// All cursor positions are char indices (not byte offsets) for Unicode safety.

pub struct InputBuffer {
    pub lines:      Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize, // char index within lines[cursor_row]
    pub scroll:     u16,   // first visible row

    // Selection anchor — None means no selection active.
    // When Some((row, col)), marks where the selection started;
    // the other end is always the current cursor position.
    pub sel_anchor: Option<(usize, usize)>,
}

impl InputBuffer {
    pub fn new() -> Self {
        Self { lines: vec![String::new()], cursor_row: 0, cursor_col: 0, scroll: 0, sel_anchor: None }
    }

    // ── Text access ──────────────────────────────────────────────────────────

    pub fn get_text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn set_text(&mut self, text: &str) {
        self.lines = text.lines().map(|l| l.to_string()).collect();
        if self.lines.is_empty() { self.lines.push(String::new()); }
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.scroll     = 0;
    }

    pub fn clear(&mut self) {
        self.lines      = vec![String::new()];
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.scroll     = 0;
    }

    // ── Editing ──────────────────────────────────────────────────────────────

    pub fn insert_char(&mut self, c: char) {
        let bp = char_to_byte(&self.lines[self.cursor_row], self.cursor_col);
        self.lines[self.cursor_row].insert(bp, c);
        self.cursor_col += 1;
    }

    pub fn insert_newline(&mut self) {
        // Split current line at cursor
        let bp    = char_to_byte(&self.lines[self.cursor_row], self.cursor_col);
        let right = self.lines[self.cursor_row][bp..].to_string();
        self.lines[self.cursor_row].truncate(bp);
        self.cursor_row += 1;
        self.lines.insert(self.cursor_row, right);
        self.cursor_col = 0;
        self.clamp_scroll_to_cursor();
    }

    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            // Remove char before cursor in the same line
            let bp   = char_to_byte(&self.lines[self.cursor_row], self.cursor_col);
            let prev = char_to_byte(&self.lines[self.cursor_row], self.cursor_col - 1);
            self.lines[self.cursor_row].drain(prev..bp);
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            // Merge current line into previous
            let cur  = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            self.cursor_col  = char_len(&self.lines[self.cursor_row]);
            self.lines[self.cursor_row].push_str(&cur);
            self.clamp_scroll_to_cursor();
        }
    }

    pub fn delete_char(&mut self) {
        let line_len = char_len(&self.lines[self.cursor_row]);
        if self.cursor_col < line_len {
            // Remove char at cursor
            let bp   = char_to_byte(&self.lines[self.cursor_row], self.cursor_col);
            let next = char_to_byte(&self.lines[self.cursor_row], self.cursor_col + 1);
            self.lines[self.cursor_row].drain(bp..next);
        } else if self.cursor_row + 1 < self.lines.len() {
            // Merge next line into current
            let next = self.lines.remove(self.cursor_row + 1);
            self.lines[self.cursor_row].push_str(&next);
        }
    }

    // ── Cursor movement ──────────────────────────────────────────────────────

    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col  = char_len(&self.lines[self.cursor_row]);
            self.clamp_scroll_to_cursor();
        }
    }

    pub fn move_right(&mut self) {
        let line_len = char_len(&self.lines[self.cursor_row]);
        if self.cursor_col < line_len {
            self.cursor_col += 1;
        } else if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col  = 0;
            self.clamp_scroll_to_cursor();
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col  = self.cursor_col.min(char_len(&self.lines[self.cursor_row]));
            self.clamp_scroll_to_cursor();
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col  = self.cursor_col.min(char_len(&self.lines[self.cursor_row]));
            self.clamp_scroll_to_cursor();
        }
    }

    pub fn move_home(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor_col = char_len(&self.lines[self.cursor_row]);
    }

    // ── Selection ────────────────────────────────────────────────────────────

    /// Start or extend a selection. Call before moving the cursor for Shift+Arrow.
    pub fn start_selection(&mut self) {
        if self.sel_anchor.is_none() {
            self.sel_anchor = Some((self.cursor_row, self.cursor_col));
        }
    }

    /// Clear selection without deleting anything.
    pub fn clear_selection(&mut self) {
        self.sel_anchor = None;
    }

    /// Returns true if a selection is active.
    pub fn has_selection(&self) -> bool {
        self.sel_anchor.is_some()
    }

    /// Normalized selection: (start_row, start_col, end_row, end_col)
    /// where start is always <= end in document order.
    pub fn selection_range(&self) -> Option<(usize, usize, usize, usize)> {
        let (ar, ac) = self.sel_anchor?;
        let (cr, cc) = (self.cursor_row, self.cursor_col);
        // Compare positions
        let (sr, sc, er, ec) = if (ar, ac) <= (cr, cc) {
            (ar, ac, cr, cc)
        } else {
            (cr, cc, ar, ac)
        };
        if (sr, sc) == (er, ec) { return None; } // zero-length selection
        Some((sr, sc, er, ec))
    }

    /// Copy selected text to a string. Returns None if no selection.
    pub fn copy_selection(&self) -> Option<String> {
        let (sr, sc, er, ec) = self.selection_range()?;
        let mut out = String::new();
        for row in sr..=er {
            let line = &self.lines[row];
            let chars: Vec<char> = line.chars().collect();
            let from = if row == sr { sc } else { 0 };
            let to   = if row == er { ec } else { chars.len() };
            let segment: String = chars[from..to].iter().collect();
            out.push_str(&segment);
            if row < er { out.push('\n'); }
        }
        Some(out)
    }

    /// Delete the selected region. Returns deleted text.
    pub fn delete_selection(&mut self) -> String {
        let deleted = self.copy_selection().unwrap_or_default();
        let range   = self.selection_range();
        self.sel_anchor = None;
        if let Some((sr, sc, er, ec)) = range {
            if sr == er {
                // Single line
                let bp_from = char_to_byte(&self.lines[sr], sc);
                let bp_to   = char_to_byte(&self.lines[sr], ec);
                self.lines[sr].drain(bp_from..bp_to);
            } else {
                // Multi-line: keep prefix of start line + suffix of end line
                let end_bp  = char_to_byte(&self.lines[er], ec);
                let suffix: String = self.lines[er][end_bp..].to_string();
                let start_bp = char_to_byte(&self.lines[sr], sc);
                self.lines[sr].truncate(start_bp);
                self.lines[sr].push_str(&suffix);
                // Remove lines between sr+1..=er
                self.lines.drain(sr + 1..=er);
            }
            self.cursor_row = sr;
            self.cursor_col = sc;
            self.clamp_scroll_to_cursor();
        }
        deleted
    }

    /// Select all text in the buffer.
    pub fn select_all(&mut self) {
        self.sel_anchor = Some((0, 0));
        let last_row = self.lines.len() - 1;
        let last_col = char_len(&self.lines[last_row]);
        self.cursor_row = last_row;
        self.cursor_col = last_col;
    }

    pub fn page_up(&mut self, page: u16) {
        self.cursor_row  = self.cursor_row.saturating_sub(page as usize);
        self.cursor_col  = self.cursor_col.min(char_len(&self.lines[self.cursor_row]));
        self.clamp_scroll_to_cursor();
    }

    pub fn page_down(&mut self, page: u16) {
        let max = self.lines.len().saturating_sub(1);
        self.cursor_row  = (self.cursor_row + page as usize).min(max);
        self.cursor_col  = self.cursor_col.min(char_len(&self.lines[self.cursor_row]));
        self.clamp_scroll_to_cursor();
    }

    // ── Scroll sync ──────────────────────────────────────────────────────────

    /// Call from the UI renderer with the actual inner pane height (rows).
    /// Adjusts `self.scroll` so the cursor is always in the visible window.
    pub fn sync_scroll(&mut self, visible_rows: u16) {
        let vis = visible_rows.max(1) as usize;
        if self.cursor_row < self.scroll as usize {
            self.scroll = self.cursor_row as u16;
        } else if self.cursor_row >= self.scroll as usize + vis {
            self.scroll = (self.cursor_row - vis + 1) as u16;
        }
    }

    // Adjust scroll so cursor is never above the top; called during edits
    // where we don't yet know viewport height.
    fn clamp_scroll_to_cursor(&mut self) {
        if self.cursor_row < self.scroll as usize {
            self.scroll = self.cursor_row as u16;
        }
    }
}

// ─── OutputBuffer ────────────────────────────────────────────────────────────

pub struct OutputBuffer {
    pub content: String,
    pub scroll:  u16,
}

impl OutputBuffer {
    pub fn new() -> Self {
        Self { content: String::new(), scroll: 0 }
    }

    /// Replace content and reset scroll to top.
    pub fn set(&mut self, content: String) {
        self.content = content;
        self.scroll  = 0;
    }

    pub fn line_count(&self) -> usize {
        if self.content.is_empty() { 0 } else { self.content.lines().count() }
    }

    // ── Scroll helpers ───────────────────────────────────────────────────────

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn scroll_down(&mut self, visible_rows: u16) {
        let max = self.max_scroll(visible_rows);
        self.scroll = (self.scroll + 1).min(max);
    }

    pub fn page_up(&mut self, page: u16) {
        self.scroll = self.scroll.saturating_sub(page);
    }

    pub fn page_down(&mut self, page: u16, visible_rows: u16) {
        let max = self.max_scroll(visible_rows);
        self.scroll = (self.scroll + page).min(max);
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll = 0;
    }

    pub fn scroll_to_bottom(&mut self, visible_rows: u16) {
        self.scroll = self.max_scroll(visible_rows);
    }

    fn max_scroll(&self, visible_rows: u16) -> u16 {
        let lines = self.line_count() as u16;
        lines.saturating_sub(visible_rows)
    }
}

// ─── App ─────────────────────────────────────────────────────────────────────

pub struct App {
    pub active_tab:       ActiveTab,
    pub focus:            Focus,

    // ── JSON tool ────────────────────────────────────────────────────────────
    pub json_mode:        JsonMode,
    pub json_input:       InputBuffer,
    pub json_output:      OutputBuffer,

    // ── ISO 8583 ─────────────────────────────────────────────────────────────
    pub iso_mode:         IsoMode,
    pub iso_input:        InputBuffer,
    pub iso_output:       OutputBuffer,

    // ── TLV / EMV decoder ────────────────────────────────────────────────────
    pub tlv_input:        InputBuffer,
    pub tlv_output:       OutputBuffer,

    // ── Key Management ───────────────────────────────────────────────────────
    pub key_op:           KeyOp,
    pub key_field:        [InputBuffer; 3], // indexed 0/1/2
    pub key_focus_field:  u8,               // which key_field has focus
    pub key_output:       OutputBuffer,

    // ── Simulator ────────────────────────────────────────────────────────────
    pub sim_mode:         SimMode,
    pub sim_port:         String,
    pub sim_host:         String,
    pub sim_framing:      String, // "binary2" | "ascii4" | "none"
    pub sim_message:      InputBuffer,
    pub sim_output:       OutputBuffer,
    pub sim_server:       SimServer,
    pub sim_state:        Arc<Mutex<SimState>>,

    // ── Settlement / Recon ───────────────────────────────────────────────────
    pub settle_input:     InputBuffer,
    pub settle_output:    OutputBuffer,

    // ── Global ───────────────────────────────────────────────────────────────
    pub clipboard:        String,   // Ctrl+C / Ctrl+X target
    pub status:           String,
    pub status_is_error:  bool,
    pub should_quit:      bool,

    // Sample cycle indices (per tab)
    pub sample_idx:       [usize; 6],
}

impl App {
    pub fn new() -> Self {
        let sim_state = Arc::new(Mutex::new(SimState::new()));
        Self {
            active_tab:      ActiveTab::Json,
            focus:           Focus::Input,

            json_mode:       JsonMode::Beautify,
            json_input:      InputBuffer::new(),
            json_output:     OutputBuffer::new(),

            iso_mode:        IsoMode::Hex,
            iso_input:       InputBuffer::new(),
            iso_output:      OutputBuffer::new(),

            tlv_input:       InputBuffer::new(),
            tlv_output:      OutputBuffer::new(),

            key_op:          KeyOp::Kcv,
            key_field:       [InputBuffer::new(), InputBuffer::new(), InputBuffer::new()],
            key_focus_field: 0,
            key_output:      OutputBuffer::new(),

            sim_mode:        SimMode::Server,
            sim_port:        "8583".to_string(),
            sim_host:        "127.0.0.1".to_string(),
            sim_framing:     "binary2".to_string(),
            sim_message:     InputBuffer::new(),
            sim_output:      OutputBuffer::new(),
            sim_server:      SimServer::new(),
            sim_state,

            settle_input:    InputBuffer::new(),
            settle_output:   OutputBuffer::new(),

            clipboard:       String::new(),
            status:          "F1-F6: tabs │ Tab: pane │ Space: run │ Shift+arrows: select │ Ctrl+C/X: copy/cut │ Ctrl+A: all │ Ctrl+Q: quit".to_string(),
            status_is_error: false,
            should_quit:     false,

            sample_idx:      [0; 6],
        }
    }

    pub fn set_status(&mut self, msg: impl Into<String>, is_error: bool) {
        self.status      = msg.into();
        self.status_is_error = is_error;
    }

    // ── Convenience accessors ─────────────────────────────────────────────────

    /// Returns a mutable reference to the currently-active input buffer.
    pub fn current_input(&mut self) -> &mut InputBuffer {
        match self.active_tab {
            ActiveTab::Json       => &mut self.json_input,
            ActiveTab::Iso8583    => &mut self.iso_input,
            ActiveTab::Tlv        => &mut self.tlv_input,
            ActiveTab::KeyMgmt    => {
                let f = self.key_focus_field as usize;
                &mut self.key_field[f.min(2)]
            }
            ActiveTab::Simulator  => &mut self.sim_message,
            ActiveTab::Settlement => &mut self.settle_input,
        }
    }

    /// Returns a mutable reference to the currently-active output buffer.
    pub fn current_output(&mut self) -> &mut OutputBuffer {
        match self.active_tab {
            ActiveTab::Json       => &mut self.json_output,
            ActiveTab::Iso8583    => &mut self.iso_output,
            ActiveTab::Tlv        => &mut self.tlv_output,
            ActiveTab::KeyMgmt    => &mut self.key_output,
            ActiveTab::Simulator  => &mut self.sim_output,
            ActiveTab::Settlement => &mut self.settle_output,
        }
    }
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

/// Convert a char-index to a byte-offset within `s`.
/// Returns `s.len()` if `char_idx` is past the end.
pub fn char_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

/// Number of chars (Unicode scalar values) in `s`.
pub fn char_len(s: &str) -> usize {
    s.chars().count()
}
