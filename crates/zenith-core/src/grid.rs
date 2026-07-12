use crate::cell::Cell;

pub struct Grid {
    cols: usize,
    rows: usize,
    cells: Vec<Cell>,
    scrollback: Vec<Vec<Cell>>,
    scrollback_limit: usize,
    trimmed: usize,
    display_offset: usize,
    dirty: Vec<bool>,
}

impl Grid {
    pub fn new(cols: usize, rows: usize, scrollback_limit: usize) -> Self {
        Self {
            cols,
            rows,
            cells: vec![Cell::default(); cols * rows],
            scrollback: Vec::new(),
            scrollback_limit,
            trimmed: 0,
            display_offset: 0,
            dirty: vec![true; rows],
        }
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cell(&self, col: usize, row: usize) -> &Cell {
        &self.cells[row * self.cols + col]
    }

    pub fn cell_mut(&mut self, col: usize, row: usize) -> &mut Cell {
        self.dirty[row] = true;
        &mut self.cells[row * self.cols + col]
    }

    pub fn row_slice(&self, row: usize) -> &[Cell] {
        let start = row * self.cols;
        &self.cells[start..start + self.cols]
    }

    pub fn is_dirty(&self, row: usize) -> bool {
        self.dirty[row]
    }

    pub fn clear_dirty(&mut self) {
        self.dirty.iter_mut().for_each(|d| *d = false);
    }

    pub fn mark_all_dirty(&mut self) {
        self.dirty.iter_mut().for_each(|d| *d = true);
    }

    pub fn scroll_up(&mut self, top: usize, bottom: usize, count: usize) {
        for _ in 0..count {
            if top == 0 {
                let row_data: Vec<Cell> = self.row_slice(top).to_vec();
                self.scrollback.push(row_data);
                if self.scrollback.len() > self.scrollback_limit {
                    self.scrollback.remove(0);
                    self.trimmed += 1;
                }
                if self.display_offset > 0 {
                    self.display_offset = (self.display_offset + 1).min(self.scrollback.len());
                }
            }
            for row in top..bottom {
                for col in 0..self.cols {
                    let src = self.cells[(row + 1) * self.cols + col];
                    self.cells[row * self.cols + col] = src;
                }
                self.dirty[row] = true;
            }
            for col in 0..self.cols {
                self.cells[bottom * self.cols + col] = Cell::default();
            }
            self.dirty[bottom] = true;
        }
    }

    pub fn scroll_down(&mut self, top: usize, bottom: usize, count: usize) {
        for _ in 0..count {
            for row in (top + 1..=bottom).rev() {
                for col in 0..self.cols {
                    let src = self.cells[(row - 1) * self.cols + col];
                    self.cells[row * self.cols + col] = src;
                }
                self.dirty[row] = true;
            }
            for col in 0..self.cols {
                self.cells[top * self.cols + col] = Cell::default();
            }
            self.dirty[top] = true;
        }
    }

    pub fn resize(&mut self, new_cols: usize, new_rows: usize) {
        let mut new_cells = vec![Cell::default(); new_cols * new_rows];
        let copy_rows = self.rows.min(new_rows);
        let copy_cols = self.cols.min(new_cols);
        for row in 0..copy_rows {
            for col in 0..copy_cols {
                new_cells[row * new_cols + col] = self.cells[row * self.cols + col];
            }
        }
        self.cells = new_cells;
        self.cols = new_cols;
        self.rows = new_rows;
        self.dirty = vec![true; new_rows];
    }

    pub fn clear(&mut self) {
        self.cells.fill(Cell::default());
        self.mark_all_dirty();
    }

    pub fn insert_chars(&mut self, row: usize, col: usize, count: usize) {
        let start = row * self.cols;
        let line = &mut self.cells[start..start + self.cols];
        let n = count.min(self.cols - col);
        line[col..].rotate_right(n);
        line[col..col + n].fill(Cell::default());
        self.dirty[row] = true;
    }

    pub fn delete_chars(&mut self, row: usize, col: usize, count: usize) {
        let start = row * self.cols;
        let line = &mut self.cells[start..start + self.cols];
        let n = count.min(self.cols - col);
        line[col..].rotate_left(n);
        let len = line.len();
        line[len - n..].fill(Cell::default());
        self.dirty[row] = true;
    }

    pub fn erase_line(&mut self, row: usize, start_col: usize, end_col: usize) {
        let end = end_col.min(self.cols);
        for col in start_col..end {
            self.cells[row * self.cols + col] = Cell::default();
        }
        self.dirty[row] = true;
    }

    pub fn scrollback_len(&self) -> usize {
        self.scrollback.len()
    }

    // Monotonic count of lines ever pushed above the screen (survives trimming).
    pub fn total_lines(&self) -> usize {
        self.trimmed + self.scrollback.len()
    }

    pub fn abs_rows_text(&self, start_abs: usize, end_abs: usize) -> String {
        let mut lines: Vec<String> = Vec::new();
        for abs in start_abs..=end_abs {
            if abs < self.trimmed {
                continue;
            }
            let idx = abs - self.trimmed;
            let mut s = String::new();
            if idx < self.scrollback.len() {
                for cell in &self.scrollback[idx] {
                    if cell.width != 0 {
                        s.push(cell.c);
                    }
                }
            } else {
                let row = idx - self.scrollback.len();
                if row >= self.rows {
                    break;
                }
                for cell in self.row_slice(row) {
                    if cell.width != 0 {
                        s.push(cell.c);
                    }
                }
            }
            lines.push(s.trim_end().to_string());
        }
        lines.join("\n")
    }

    pub fn display_offset(&self) -> usize {
        self.display_offset
    }

    pub fn scroll_display(&mut self, delta: i32) {
        let new = (self.display_offset as i64 + delta as i64)
            .clamp(0, self.scrollback.len() as i64) as usize;
        if new != self.display_offset {
            self.display_offset = new;
            self.mark_all_dirty();
        }
    }

    pub fn reset_display_offset(&mut self) {
        if self.display_offset != 0 {
            self.display_offset = 0;
            self.mark_all_dirty();
        }
    }

    pub fn display_cell(&self, col: usize, row: usize) -> Cell {
        if row >= self.display_offset {
            self.cells[(row - self.display_offset) * self.cols + col]
        } else {
            let line = &self.scrollback[self.scrollback.len() - self.display_offset + row];
            line.get(col).copied().unwrap_or_default()
        }
    }

    pub fn display_text_range(&self, start: (usize, usize), end: (usize, usize)) -> String {
        let (start, end) = if (start.1, start.0) <= (end.1, end.0) {
            (start, end)
        } else {
            (end, start)
        };

        let cols = self.cols;
        let mut rows_text: Vec<String> = Vec::new();

        for row in start.1..=end.1 {
            let col_start = if row == start.1 { start.0.min(cols.saturating_sub(1)) } else { 0 };
            let col_end = if row == end.1 { end.0.min(cols.saturating_sub(1)) } else { cols.saturating_sub(1) };

            let mut s = String::new();
            for col in col_start..=col_end {
                let cell = self.display_cell(col, row);
                if cell.width != 0 {
                    s.push(cell.c);
                }
            }
            rows_text.push(s.trim_end().to_string());
        }

        rows_text.join("\n")
    }

    pub fn screen_text(&self, scrollback_lines: usize) -> String {
        let n = scrollback_lines.min(self.scrollback.len());
        let mut lines: Vec<String> = Vec::with_capacity(n + self.rows);
        for line in &self.scrollback[self.scrollback.len() - n..] {
            let mut s = String::new();
            for cell in line {
                if cell.width != 0 {
                    s.push(cell.c);
                }
            }
            lines.push(s.trim_end().to_string());
        }
        for row in 0..self.rows {
            let mut s = String::new();
            for cell in self.row_slice(row) {
                if cell.width != 0 {
                    s.push(cell.c);
                }
            }
            lines.push(s.trim_end().to_string());
        }
        lines.join("\n")
    }
}
