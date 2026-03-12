/// Which tab is active
#[derive(Clone, Copy, PartialEq)]
pub enum ActiveTab {
    Json,
    Iso8583,
}

/// Which pane has focus
#[derive(Clone, Copy, PartialEq)]
pub enum Focus {
    Input,
    Output,
}

/// JSON editing mode
#[derive(Clone, Copy, PartialEq)]
pub enum JsonMode {
    Beautify,
    Minify,
}

/// ISO 8583 decode mode
#[derive(Clone, Copy, PartialEq)]
pub enum IsoMode {
    /// Fully hex-encoded: every byte = 2 hex chars (e.g. MTI "0200" → "30323030")
    Hex,
    /// Raw/ASCII: MTI literal "0200", bitmap=16 hex chars, field data=ASCII
    Raw,
}

/// Multi-line input buffer with cursor tracking
pub struct InputBuffer {
    pub lines: Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub scroll: u16,
}

impl InputBuffer {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            scroll: 0,
        }
    }

    pub fn insert_char(&mut self, c: char) {
        // Handle char boundaries safely
        let line = &mut self.lines[self.cursor_row];
        // cursor_col is a byte offset, ensure it's on a char boundary
        let byte_pos = char_byte_pos(line, self.cursor_col);
        line.insert(byte_pos, c);
        self.cursor_col += 1;
    }

    pub fn insert_newline(&mut self) {
        let line = self.lines[self.cursor_row].clone();
        let byte_pos = char_byte_pos(&line, self.cursor_col);
        let left = line[..byte_pos].to_string();
        let right = line[byte_pos..].to_string();
        self.lines[self.cursor_row] = left;
        self.lines.insert(self.cursor_row + 1, right);
        self.cursor_row += 1;
        self.cursor_col = 0;
        self.adjust_scroll();
    }

    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            let line = &mut self.lines[self.cursor_row];
            let byte_pos = char_byte_pos(line, self.cursor_col);
            // Remove char before cursor
            let prev_byte = char_byte_pos(line, self.cursor_col - 1);
            line.drain(prev_byte..byte_pos);
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            let current = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            let prev_len = char_count(&self.lines[self.cursor_row]);
            self.lines[self.cursor_row].push_str(&current);
            self.cursor_col = prev_len;
            self.adjust_scroll();
        }
    }

    pub fn delete_char(&mut self) {
        let line_len = char_count(&self.lines[self.cursor_row]);
        if self.cursor_col < line_len {
            let line = &mut self.lines[self.cursor_row];
            let byte_pos = char_byte_pos(line, self.cursor_col);
            let next_byte = char_byte_pos(line, self.cursor_col + 1);
            line.drain(byte_pos..next_byte);
        } else if self.cursor_row < self.lines.len() - 1 {
            let next = self.lines.remove(self.cursor_row + 1);
            self.lines[self.cursor_row].push_str(&next);
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = char_count(&self.lines[self.cursor_row]);
            self.adjust_scroll();
        }
    }

    pub fn move_right(&mut self) {
        let line_len = char_count(&self.lines[self.cursor_row]);
        if self.cursor_col < line_len {
            self.cursor_col += 1;
        } else if self.cursor_row < self.lines.len() - 1 {
            self.cursor_row += 1;
            self.cursor_col = 0;
            self.adjust_scroll();
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            let line_len = char_count(&self.lines[self.cursor_row]);
            self.cursor_col = self.cursor_col.min(line_len);
            self.adjust_scroll();
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor_row < self.lines.len() - 1 {
            self.cursor_row += 1;
            let line_len = char_count(&self.lines[self.cursor_row]);
            self.cursor_col = self.cursor_col.min(line_len);
            self.adjust_scroll();
        }
    }

    pub fn move_home(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor_col = char_count(&self.lines[self.cursor_row]);
    }

    pub fn page_up(&mut self, page_size: u16) {
        let jump = page_size as usize;
        if self.cursor_row >= jump {
            self.cursor_row -= jump;
        } else {
            self.cursor_row = 0;
        }
        let line_len = char_count(&self.lines[self.cursor_row]);
        self.cursor_col = self.cursor_col.min(line_len);
        self.adjust_scroll();
    }

    pub fn page_down(&mut self, page_size: u16) {
        let jump = page_size as usize;
        let max_row = self.lines.len().saturating_sub(1);
        self.cursor_row = (self.cursor_row + jump).min(max_row);
        let line_len = char_count(&self.lines[self.cursor_row]);
        self.cursor_col = self.cursor_col.min(line_len);
        self.adjust_scroll();
    }

    pub fn clear(&mut self) {
        self.lines = vec![String::new()];
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.scroll = 0;
    }

    pub fn get_text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn set_text(&mut self, text: &str) {
        self.lines = text.lines().map(|l| l.to_string()).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.scroll = 0;
    }

    fn adjust_scroll(&mut self) {
        // Keep cursor visible (adjusted in UI based on area height)
        if self.cursor_row < self.scroll as usize {
            self.scroll = self.cursor_row as u16;
        }
    }

    pub fn sync_scroll(&mut self, area_height: u16) {
        let visible_rows = area_height.saturating_sub(2) as usize; // subtract borders
        if self.cursor_row < self.scroll as usize {
            self.scroll = self.cursor_row as u16;
        } else if self.cursor_row >= self.scroll as usize + visible_rows {
            self.scroll = (self.cursor_row - visible_rows + 1) as u16;
        }
    }
}

/// Get byte position of the nth character in a string
fn char_byte_pos(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

/// Count characters (not bytes) in a string
fn char_count(s: &str) -> usize {
    s.chars().count()
}

/// Scrollable output buffer
pub struct OutputBuffer {
    pub content: String,
    pub scroll: u16,
}

impl OutputBuffer {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            scroll: 0,
        }
    }

    pub fn set(&mut self, content: String) {
        self.content = content;
        self.scroll = 0;
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn scroll_down(&mut self, max_lines: usize, view_height: u16) {
        let max_scroll = (max_lines as u16).saturating_sub(view_height.saturating_sub(2));
        if self.scroll < max_scroll {
            self.scroll += 1;
        }
    }

    pub fn page_up(&mut self, page_size: u16) {
        self.scroll = self.scroll.saturating_sub(page_size);
    }

    pub fn page_down(&mut self, page_size: u16, max_lines: usize, view_height: u16) {
        let max_scroll = (max_lines as u16).saturating_sub(view_height.saturating_sub(2));
        self.scroll = (self.scroll + page_size).min(max_scroll);
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll = 0;
    }

    pub fn scroll_to_bottom(&mut self, max_lines: usize, view_height: u16) {
        let max_scroll = (max_lines as u16).saturating_sub(view_height.saturating_sub(2));
        self.scroll = max_scroll;
    }
}

/// Main application state
pub struct App {
    pub active_tab: ActiveTab,
    pub focus: Focus,
    pub json_mode: JsonMode,
    pub iso_mode: IsoMode,

    // JSON tool
    pub json_input: InputBuffer,
    pub json_output: OutputBuffer,

    // ISO 8583 tool
    pub iso_input: InputBuffer,
    pub iso_output: OutputBuffer,

    // Status bar message
    pub status: String,
    pub status_is_error: bool,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            active_tab: ActiveTab::Json,
            focus: Focus::Input,
            json_mode: JsonMode::Beautify,
            iso_mode: IsoMode::Hex,

            json_input: InputBuffer::new(),
            json_output: OutputBuffer::new(),

            iso_input: InputBuffer::new(),
            iso_output: OutputBuffer::new(),

            status: "Press F1/F2 to switch tabs | Tab to switch pane | F5 to process | Ctrl+Q to quit".to_string(),
            status_is_error: false,
            should_quit: false,
        }
    }

    pub fn set_status(&mut self, msg: impl Into<String>, is_error: bool) {
        self.status = msg.into();
        self.status_is_error = is_error;
    }

    pub fn current_input(&mut self) -> &mut InputBuffer {
        match self.active_tab {
            ActiveTab::Json => &mut self.json_input,
            ActiveTab::Iso8583 => &mut self.iso_input,
        }
    }

    pub fn current_output(&mut self) -> &mut OutputBuffer {
        match self.active_tab {
            ActiveTab::Json => &mut self.json_output,
            ActiveTab::Iso8583 => &mut self.iso_output,
        }
    }
}
