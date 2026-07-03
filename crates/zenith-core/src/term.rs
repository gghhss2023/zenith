use crate::cell::{CellAttrs, Color, ANSI_COLORS, DEFAULT_BG, DEFAULT_FG};
use crate::grid::Grid;
use vte::{Params, Parser, Perform};

#[derive(Debug, Clone, Copy, PartialEq)]
enum ShellState {
    Ground,
    Prompt,
    Input,
    Running,
}

struct TerminalState {
    grid: Grid,
    alt_grid: Option<Grid>,
    cursor_col: usize,
    cursor_row: usize,
    saved_cursor_col: usize,
    saved_cursor_row: usize,
    current_attrs: CellAttrs,
    current_fg: Color,
    current_bg: Color,
    scroll_top: usize,
    scroll_bottom: usize,
    title: String,
    cwd: String,
    show_cursor: bool,
    auto_wrap: bool,
    pending_wrap: bool,
    last_char: Option<char>,
    shell_state: ShellState,
    input_start: Option<(usize, usize)>,
    pending_command: Option<String>,
    completed_commands: Vec<String>,
}

pub struct Terminal {
    state: TerminalState,
    parser: Parser,
}

impl Terminal {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            state: TerminalState {
                grid: Grid::new(cols, rows, 10_000),
                alt_grid: None,
                cursor_col: 0,
                cursor_row: 0,
                saved_cursor_col: 0,
                saved_cursor_row: 0,
                current_attrs: CellAttrs::empty(),
                current_fg: DEFAULT_FG,
                current_bg: DEFAULT_BG,
                scroll_top: 0,
                scroll_bottom: rows.saturating_sub(1),
                title: String::from("Zenith"),
                cwd: String::new(),
                show_cursor: true,
                auto_wrap: true,
                pending_wrap: false,
                last_char: None,
                shell_state: ShellState::Ground,
                input_start: None,
                pending_command: None,
                completed_commands: Vec::new(),
            },
            parser: Parser::new(),
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.parser.advance(&mut self.state, byte);
        }
    }

    pub fn grid(&self) -> &Grid {
        &self.state.grid
    }

    pub fn cursor(&self) -> (usize, usize) {
        (self.state.cursor_col, self.state.cursor_row)
    }

    pub fn show_cursor(&self) -> bool {
        self.state.show_cursor
    }

    pub fn is_alt_screen(&self) -> bool {
        self.state.alt_grid.is_some()
    }

    pub fn title(&self) -> &str {
        &self.state.title
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.state.grid.resize(cols, rows);
        if let Some(ref mut alt) = self.state.alt_grid {
            alt.resize(cols, rows);
        }
        self.state.scroll_bottom = rows.saturating_sub(1);
        if cols > 0 {
            self.state.cursor_col = self.state.cursor_col.min(cols - 1);
        }
        if rows > 0 {
            self.state.cursor_row = self.state.cursor_row.min(rows - 1);
        }
        self.state.grid.mark_all_dirty();
    }

    pub fn clear_dirty(&mut self) {
        self.state.grid.clear_dirty();
    }

    pub fn scroll_display(&mut self, delta: i32) {
        self.state.grid.scroll_display(delta);
    }

    pub fn reset_display_offset(&mut self) {
        self.state.grid.reset_display_offset();
    }

    pub fn current_input(&self) -> Option<String> {
        if self.state.shell_state != ShellState::Input {
            return None;
        }
        let (col, abs_row) = self.state.input_start?;
        for c in self.state.cursor_col..self.state.grid.cols() {
            let cell = self.state.grid.cell(c, self.state.cursor_row);
            if cell.width != 0 && cell.c != ' ' && cell.c != '\0' {
                return None;
            }
        }
        let cur_abs = self.state.grid.scrollback_len() + self.state.cursor_row;
        self.state
            .extract_text(col, abs_row, self.state.cursor_col, cur_abs)
    }

    pub fn take_completed_command(&mut self) -> Option<String> {
        if self.state.completed_commands.is_empty() {
            None
        } else {
            Some(self.state.completed_commands.remove(0))
        }
    }
}

impl TerminalState {
    fn write_char(&mut self, c: char) {
        let width = unicode_width::UnicodeWidthChar::width(c).unwrap_or(1);

        if self.pending_wrap && self.auto_wrap {
            self.cursor_col = 0;
            self.linefeed();
            self.pending_wrap = false;
        }

        if self.cursor_col + width > self.grid.cols() {
            if self.auto_wrap {
                self.cursor_col = 0;
                self.linefeed();
            } else {
                return;
            }
        }

        let cell = self.grid.cell_mut(self.cursor_col, self.cursor_row);
        cell.c = c;
        cell.fg = self.current_fg;
        cell.bg = self.current_bg;
        cell.attrs = self.current_attrs;
        cell.width = width as u8;

        for i in 1..width {
            if self.cursor_col + i < self.grid.cols() {
                let pad = self.grid.cell_mut(self.cursor_col + i, self.cursor_row);
                pad.c = ' ';
                pad.width = 0;
                pad.fg = self.current_fg;
                pad.bg = self.current_bg;
                pad.attrs = self.current_attrs;
            }
        }

        self.cursor_col += width;
        if self.cursor_col >= self.grid.cols() {
            self.pending_wrap = true;
            self.cursor_col = self.grid.cols() - 1;
        }
        self.last_char = Some(c);
    }

    fn linefeed(&mut self) {
        if self.cursor_row == self.scroll_bottom {
            self.grid.scroll_up(self.scroll_top, self.scroll_bottom, 1);
        } else if self.cursor_row < self.grid.rows() - 1 {
            self.cursor_row += 1;
        }
    }

    fn reverse_index(&mut self) {
        if self.cursor_row == self.scroll_top {
            self.grid.scroll_down(self.scroll_top, self.scroll_bottom, 1);
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
        }
    }

    fn enter_alt_screen(&mut self) {
        self.shell_state = ShellState::Ground;
        self.input_start = None;
        let cols = self.grid.cols();
        let rows = self.grid.rows();
        let main = std::mem::replace(&mut self.grid, Grid::new(cols, rows, 0));
        self.alt_grid = Some(main);
    }

    fn exit_alt_screen(&mut self) {
        if let Some(main) = self.alt_grid.take() {
            self.grid = main;
            self.grid.mark_all_dirty();
        }
    }

    fn handle_shell_marker(&mut self, param: &[u8]) {
        match param.first() {
            Some(b'A') => {
                self.shell_state = ShellState::Prompt;
                self.input_start = None;
            }
            Some(b'B') => {
                self.shell_state = ShellState::Input;
                self.input_start =
                    Some((self.cursor_col, self.grid.scrollback_len() + self.cursor_row));
            }
            Some(b'C') => {
                if let Some((col, abs_row)) = self.input_start {
                    let end_abs = self.grid.scrollback_len() + self.cursor_row;
                    self.pending_command =
                        self.extract_text(col, abs_row, self.cursor_col, end_abs);
                }
                self.shell_state = ShellState::Running;
                self.input_start = None;
            }
            Some(b'D') => {
                if let Some(cmd) = self.pending_command.take() {
                    let cmd = cmd.trim().to_string();
                    if !cmd.is_empty() {
                        self.completed_commands.push(cmd);
                    }
                }
                self.shell_state = ShellState::Ground;
                self.input_start = None;
            }
            _ => {}
        }
    }

    // Positions are (col, abs_row) where abs_row = scrollback_len + screen_row.
    // end_col is exclusive. Returns None if the start row scrolled out of the grid.
    fn extract_text(
        &self,
        start_col: usize,
        start_abs_row: usize,
        end_col: usize,
        end_abs_row: usize,
    ) -> Option<String> {
        let sb = self.grid.scrollback_len();
        if start_abs_row < sb || end_abs_row < start_abs_row {
            return None;
        }
        let mut text = String::new();
        for abs_row in start_abs_row..=end_abs_row {
            let row = abs_row - sb;
            if row >= self.grid.rows() {
                break;
            }
            let from = if abs_row == start_abs_row { start_col } else { 0 };
            let to = if abs_row == end_abs_row {
                end_col.min(self.grid.cols())
            } else {
                self.grid.cols()
            };
            let mut line = String::new();
            for col in from..to {
                let cell = self.grid.cell(col, row);
                if cell.width != 0 {
                    line.push(cell.c);
                }
            }
            if abs_row == end_abs_row {
                text.push_str(&line);
            } else {
                text.push_str(line.trim_end());
            }
        }
        Some(text)
    }

    fn parse_color_from_params(params: &[u16], idx: &mut usize) -> Option<Color> {
        if *idx >= params.len() {
            return None;
        }
        match params[*idx] {
            2 => {
                *idx += 1;
                if *idx + 2 < params.len() {
                    let r = params[*idx] as u8;
                    let g = params[*idx + 1] as u8;
                    let b = params[*idx + 2] as u8;
                    *idx += 3;
                    Some(Color::rgb(r, g, b))
                } else {
                    None
                }
            }
            5 => {
                *idx += 1;
                if *idx < params.len() {
                    let n = params[*idx] as usize;
                    *idx += 1;
                    if n < 16 {
                        Some(ANSI_COLORS[n])
                    } else if n < 232 {
                        let n = n - 16;
                        let r = (n / 36) * 51;
                        let g = ((n % 36) / 6) * 51;
                        let b = (n % 6) * 51;
                        Some(Color::rgb(r as u8, g as u8, b as u8))
                    } else {
                        let gray = ((n - 232) * 10 + 8) as u8;
                        Some(Color::rgb(gray, gray, gray))
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

impl Perform for TerminalState {
    fn print(&mut self, c: char) {
        self.write_char(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' | 0x0b | 0x0c => self.linefeed(),
            b'\r' => {
                self.cursor_col = 0;
                self.pending_wrap = false;
            }
            b'\t' => {
                let next_tab = (self.cursor_col / 8 + 1) * 8;
                self.cursor_col = next_tab.min(self.grid.cols() - 1);
            }
            0x08 if self.cursor_col > 0 => {
                self.cursor_col -= 1;
                self.pending_wrap = false;
            }
            _ => {}
        }
    }

    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _action: char) {}
    fn put(&mut self, _byte: u8) {}
    fn unhook(&mut self) {}

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.is_empty() {
            return;
        }
        match params[0] {
            b"0" | b"2" => {
                if params.len() > 1 {
                    self.title = String::from_utf8_lossy(params[1]).into();
                }
            }
            b"7" => {
                if params.len() > 1 {
                    self.cwd = String::from_utf8_lossy(params[1]).into();
                }
            }
            b"133" if params.len() > 1 => {
                self.handle_shell_marker(params[1]);
            }
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        let ps: Vec<u16> = params.iter().map(|p| p[0]).collect();
        let p0 = ps.first().copied().unwrap_or(0);
        let p1 = ps.get(1).copied().unwrap_or(0);

        if intermediates.first() == Some(&b'?') {
            match action {
                'h' => {
                    match p0 {
                        25 => self.show_cursor = true,
                        1049 => self.enter_alt_screen(),
                        7 => self.auto_wrap = true,
                        _ => {}
                    }
                    return;
                }
                'l' => {
                    match p0 {
                        25 => self.show_cursor = false,
                        1049 => self.exit_alt_screen(),
                        7 => self.auto_wrap = false,
                        _ => {}
                    }
                    return;
                }
                _ => {}
            }
        }

        match action {
            'A' => {
                let n = if p0 == 0 { 1 } else { p0 as usize };
                self.cursor_row = self.cursor_row.saturating_sub(n);
            }
            'B' => {
                let n = if p0 == 0 { 1 } else { p0 as usize };
                self.cursor_row = (self.cursor_row + n).min(self.grid.rows() - 1);
            }
            'C' => {
                let n = if p0 == 0 { 1 } else { p0 as usize };
                self.cursor_col = (self.cursor_col + n).min(self.grid.cols() - 1);
                self.pending_wrap = false;
            }
            'D' => {
                let n = if p0 == 0 { 1 } else { p0 as usize };
                self.cursor_col = self.cursor_col.saturating_sub(n);
                self.pending_wrap = false;
            }
            'H' | 'f' => {
                let row = if p0 == 0 { 1 } else { p0 as usize };
                let col = if p1 == 0 { 1 } else { p1 as usize };
                self.cursor_row = (row - 1).min(self.grid.rows() - 1);
                self.cursor_col = (col - 1).min(self.grid.cols() - 1);
                self.pending_wrap = false;
            }
            'J' => {
                match p0 {
                    0 => {
                        self.grid.erase_line(self.cursor_row, self.cursor_col, self.grid.cols());
                        for row in (self.cursor_row + 1)..self.grid.rows() {
                            self.grid.erase_line(row, 0, self.grid.cols());
                        }
                    }
                    1 => {
                        for row in 0..self.cursor_row {
                            self.grid.erase_line(row, 0, self.grid.cols());
                        }
                        self.grid.erase_line(self.cursor_row, 0, self.cursor_col + 1);
                    }
                    2 | 3 => self.grid.clear(),
                    _ => {}
                }
            }
            'K' => match p0 {
                0 => self.grid.erase_line(self.cursor_row, self.cursor_col, self.grid.cols()),
                1 => self.grid.erase_line(self.cursor_row, 0, self.cursor_col + 1),
                2 => self.grid.erase_line(self.cursor_row, 0, self.grid.cols()),
                _ => {}
            },
            'L' => {
                let n = if p0 == 0 { 1 } else { p0 as usize };
                self.grid.scroll_down(self.cursor_row, self.scroll_bottom, n);
            }
            'M' => {
                let n = if p0 == 0 { 1 } else { p0 as usize };
                self.grid.scroll_up(self.cursor_row, self.scroll_bottom, n);
            }
            '@' => {
                let n = if p0 == 0 { 1 } else { p0 as usize };
                self.grid.insert_chars(self.cursor_row, self.cursor_col, n);
            }
            'P' => {
                let n = if p0 == 0 { 1 } else { p0 as usize };
                self.grid.delete_chars(self.cursor_row, self.cursor_col, n);
            }
            'X' => {
                let n = if p0 == 0 { 1 } else { p0 as usize };
                self.grid.erase_line(self.cursor_row, self.cursor_col, self.cursor_col + n);
            }
            'b' => {
                let n = if p0 == 0 { 1 } else { p0 as usize };
                if let Some(c) = self.last_char {
                    for _ in 0..n {
                        self.write_char(c);
                    }
                }
            }
            's' => {
                self.saved_cursor_col = self.cursor_col;
                self.saved_cursor_row = self.cursor_row;
            }
            'u' => {
                self.cursor_col = self.saved_cursor_col;
                self.cursor_row = self.saved_cursor_row;
                self.pending_wrap = false;
            }
            'S' => {
                let n = if p0 == 0 { 1 } else { p0 as usize };
                self.grid.scroll_up(self.scroll_top, self.scroll_bottom, n);
            }
            'T' => {
                let n = if p0 == 0 { 1 } else { p0 as usize };
                self.grid.scroll_down(self.scroll_top, self.scroll_bottom, n);
            }
            'd' => {
                let row = if p0 == 0 { 1 } else { p0 as usize };
                self.cursor_row = (row - 1).min(self.grid.rows() - 1);
            }
            'G' | '`' => {
                let col = if p0 == 0 { 1 } else { p0 as usize };
                self.cursor_col = (col - 1).min(self.grid.cols() - 1);
            }
            'r' => {
                let top = if p0 == 0 { 1 } else { p0 as usize };
                let bottom = if p1 == 0 { self.grid.rows() as u16 } else { p1 } as usize;
                self.scroll_top = (top - 1).min(self.grid.rows() - 1);
                self.scroll_bottom = (bottom - 1).min(self.grid.rows() - 1);
                self.cursor_col = 0;
                self.cursor_row = 0;
            }
            'm' => {
                if ps.is_empty() {
                    self.current_attrs = CellAttrs::empty();
                    self.current_fg = DEFAULT_FG;
                    self.current_bg = DEFAULT_BG;
                    return;
                }
                let mut i = 0;
                while i < ps.len() {
                    match ps[i] {
                        0 => {
                            self.current_attrs = CellAttrs::empty();
                            self.current_fg = DEFAULT_FG;
                            self.current_bg = DEFAULT_BG;
                        }
                        1 => self.current_attrs.insert(CellAttrs::BOLD),
                        2 => self.current_attrs.insert(CellAttrs::DIM),
                        3 => self.current_attrs.insert(CellAttrs::ITALIC),
                        4 => self.current_attrs.insert(CellAttrs::UNDERLINE),
                        7 => self.current_attrs.insert(CellAttrs::INVERSE),
                        8 => self.current_attrs.insert(CellAttrs::HIDDEN),
                        9 => self.current_attrs.insert(CellAttrs::STRIKETHROUGH),
                        22 => {
                            self.current_attrs.remove(CellAttrs::BOLD);
                            self.current_attrs.remove(CellAttrs::DIM);
                        }
                        23 => self.current_attrs.remove(CellAttrs::ITALIC),
                        24 => self.current_attrs.remove(CellAttrs::UNDERLINE),
                        27 => self.current_attrs.remove(CellAttrs::INVERSE),
                        28 => self.current_attrs.remove(CellAttrs::HIDDEN),
                        29 => self.current_attrs.remove(CellAttrs::STRIKETHROUGH),
                        30..=37 => self.current_fg = ANSI_COLORS[(ps[i] - 30) as usize],
                        38 => {
                            i += 1;
                            if let Some(c) = Self::parse_color_from_params(&ps, &mut i) {
                                self.current_fg = c;
                            }
                            continue;
                        }
                        39 => self.current_fg = DEFAULT_FG,
                        40..=47 => self.current_bg = ANSI_COLORS[(ps[i] - 40) as usize],
                        48 => {
                            i += 1;
                            if let Some(c) = Self::parse_color_from_params(&ps, &mut i) {
                                self.current_bg = c;
                            }
                            continue;
                        }
                        49 => self.current_bg = DEFAULT_BG,
                        90..=97 => self.current_fg = ANSI_COLORS[(ps[i] - 90 + 8) as usize],
                        100..=107 => self.current_bg = ANSI_COLORS[(ps[i] - 100 + 8) as usize],
                        _ => {}
                    }
                    i += 1;
                }
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        match byte {
            b'M' => self.reverse_index(),
            b'D' => self.linefeed(),
            b'E' => {
                self.cursor_col = 0;
                self.linefeed();
            }
            b'7' => {
                self.saved_cursor_col = self.cursor_col;
                self.saved_cursor_row = self.cursor_row;
            }
            b'8' => {
                self.cursor_col = self.saved_cursor_col;
                self.cursor_row = self.saved_cursor_row;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_text(term: &Terminal, row: usize) -> String {
        (0..term.grid().cols())
            .map(|col| term.grid().cell(col, row).c)
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    #[test]
    fn ich_shifts_right() {
        let mut t = Terminal::new(20, 5);
        t.feed(b"abcdef\r\x1b[2@");
        assert_eq!(row_text(&t, 0), "  abcdef");
        assert_eq!(t.cursor(), (0, 0));
    }

    #[test]
    fn dch_shifts_left() {
        let mut t = Terminal::new(20, 5);
        t.feed(b"abcdef\r\x1b[2P");
        assert_eq!(row_text(&t, 0), "cdef");
    }

    #[test]
    fn ech_erases_in_place() {
        let mut t = Terminal::new(20, 5);
        t.feed(b"abcdef\r\x1b[2X");
        assert_eq!(row_text(&t, 0), "  cdef");
    }

    #[test]
    fn mid_line_insert_then_type() {
        // simulates readline inserting 'X' between "ab" and "cd"
        let mut t = Terminal::new(20, 5);
        t.feed(b"abcd\x1b[2D\x1b[@X");
        assert_eq!(row_text(&t, 0), "abXcd");
        assert_eq!(t.cursor(), (3, 0));
    }

    #[test]
    fn rep_repeats_last_char() {
        let mut t = Terminal::new(20, 5);
        t.feed(b"-\x1b[4b");
        assert_eq!(row_text(&t, 0), "-----");
    }

    #[test]
    fn selection_single_row() {
        let mut t = Terminal::new(20, 5);
        t.feed(b"hello world");
        assert_eq!(t.grid().display_text_range((0, 0), (4, 0)), "hello");
    }

    #[test]
    fn selection_multi_row() {
        let mut t = Terminal::new(20, 5);
        t.feed(b"abc\r\ndef");
        // row 0: "abc" then spaces; row 1: "def" then spaces
        // select col 1 row 0 to col 1 row 1
        // row 0: cols 1..=19 → "bc" (trimmed)
        // row 1: cols 0..=1  → "de"
        assert_eq!(t.grid().display_text_range((1, 0), (1, 1)), "bc\nde");
    }

    #[test]
    fn selection_reversed_endpoints() {
        let mut t = Terminal::new(20, 5);
        t.feed(b"hello world");
        assert_eq!(
            t.grid().display_text_range((4, 0), (0, 0)),
            t.grid().display_text_range((0, 0), (4, 0))
        );
    }

    #[test]
    fn selection_trailing_whitespace_trimmed() {
        let mut t = Terminal::new(20, 5);
        t.feed(b"hi");
        // select entire first row — trailing spaces should be trimmed
        let result = t.grid().display_text_range((0, 0), (19, 0));
        assert_eq!(result, "hi");
    }

    #[test]
    fn screen_text_includes_scrollback_and_screen() {
        let mut term = Terminal::new(10, 2);
        term.feed(b"one\r\ntwo\r\nthree\r\nfour");
        // 2-row grid: "one" and "two" scrolled into scrollback,
        // visible screen shows "three" and "four"
        assert_eq!(term.grid().screen_text(50), "one\ntwo\nthree\nfour");
        // scrollback limited to last 1 line
        assert_eq!(term.grid().screen_text(1), "two\nthree\nfour");
    }

    #[test]
    fn osc133_captures_completed_command() {
        let mut t = Terminal::new(40, 5);
        t.feed(b"\x1b]133;A\x07$ \x1b]133;B\x07ls -la\r\n\x1b]133;C\x07output\r\n\x1b]133;D;0\x07");
        assert_eq!(t.take_completed_command(), Some("ls -la".to_string()));
        assert_eq!(t.take_completed_command(), None);
    }

    #[test]
    fn osc133_d_without_c_records_nothing() {
        let mut t = Terminal::new(40, 5);
        t.feed(b"\x1b]133;D;0\x07\x1b]133;A\x07$ ");
        assert_eq!(t.take_completed_command(), None);
    }

    #[test]
    fn current_input_tracks_typed_text() {
        let mut t = Terminal::new(40, 5);
        t.feed(b"\x1b]133;A\x07$ \x1b]133;B\x07git sta");
        assert_eq!(t.current_input(), Some("git sta".to_string()));
    }

    #[test]
    fn current_input_preserves_trailing_space() {
        let mut t = Terminal::new(40, 5);
        t.feed(b"\x1b]133;A\x07$ \x1b]133;B\x07git ");
        assert_eq!(t.current_input(), Some("git ".to_string()));
    }

    #[test]
    fn current_input_none_outside_input_state() {
        let mut t = Terminal::new(40, 5);
        assert_eq!(t.current_input(), None);
        t.feed(b"\x1b]133;A\x07$ ");
        assert_eq!(t.current_input(), None);
        t.feed(b"\x1b]133;B\x07make\r\n\x1b]133;C\x07");
        assert_eq!(t.current_input(), None); // Running state
    }

    #[test]
    fn current_input_none_when_text_right_of_cursor() {
        let mut t = Terminal::new(40, 5);
        // type "git status", then cursor-left 4 → "atus" sits right of cursor
        t.feed(b"\x1b]133;A\x07$ \x1b]133;B\x07git status\x1b[4D");
        assert_eq!(t.current_input(), None);
    }

    #[test]
    fn current_input_cleared_by_alt_screen() {
        let mut t = Terminal::new(40, 5);
        t.feed(b"\x1b]133;A\x07$ \x1b]133;B\x07vim notes");
        assert_eq!(t.current_input(), Some("vim notes".to_string()));
        t.feed(b"\x1b[?1049h");
        assert_eq!(t.current_input(), None);
    }

    #[test]
    fn is_alt_screen_tracks_1049() {
        let mut t = Terminal::new(10, 4);
        assert!(!t.is_alt_screen());
        t.feed(b"\x1b[?1049h");
        assert!(t.is_alt_screen());
        t.feed(b"\x1b[?1049l");
        assert!(!t.is_alt_screen());
    }

    #[test]
    fn osc133_command_captured_after_scroll() {
        let mut t = Terminal::new(20, 3);
        t.feed(b"x\r\ny\r\n"); // prompt lands on bottom row
        t.feed(b"\x1b]133;A\x07$ \x1b]133;B\x07echo hi\r\n\x1b]133;C\x07hi\r\n\x1b]133;D;0\x07");
        assert_eq!(t.take_completed_command(), Some("echo hi".to_string()));
    }
}
