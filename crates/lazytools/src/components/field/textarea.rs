//! TextArea tự viết — `tui-textarea 0.7` chỉ hỗ trợ `ratatui ^0.29`.
//!
//! **Phạm vi cố tình giới hạn:** insert / xóa / di chuyển con trỏ / bracketed
//! paste / soft wrap + scroll dọc. **Không** undo/redo, **không** selection,
//! **không** tìm kiếm. Người dùng paste rồi xem kết quả chứ không soạn thảo nặng
//! ở đây; mở rộng phạm vi là cách nhanh nhất làm phase này trượt lịch.

use std::cell::Cell;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Một dòng hiển thị sau khi soft wrap: `(chỉ số dòng logic, grapheme bắt đầu, kết thúc)`.
type VisualRow = (usize, usize, usize);

#[derive(Debug, Clone)]
pub struct TextArea {
    lines: Vec<String>,
    /// Con trỏ tính theo **grapheme cluster**, không phải byte.
    cursor_line: usize,
    cursor_col: usize,
    /// `Cell` để `render()` cập nhật được scroll mà vẫn giữ `draw(&self)`
    /// của `DrawableComponent`.
    scroll: Cell<usize>,
    single_line: bool,
}

fn graphemes(s: &str) -> Vec<&str> {
    s.graphemes(true).collect()
}

/// Vị trí byte của grapheme thứ `n` — dùng để cắt chuỗi mà không vỡ UTF-8.
fn byte_at(s: &str, n: usize) -> usize {
    s.grapheme_indices(true)
        .nth(n)
        .map_or_else(|| s.len(), |(i, _)| i)
}

impl TextArea {
    pub fn new(single_line: bool) -> Self {
        Self {
            lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
            scroll: Cell::new(0),
            single_line,
        }
    }

    pub fn value(&self) -> String {
        self.lines.join("\n")
    }

    pub fn set_value(&mut self, text: &str) {
        self.lines = if self.single_line {
            vec![text.replace(['\n', '\r'], " ")]
        } else {
            text.split('\n').map(|l| l.replace('\r', "")).collect()
        };
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.clamp_cursor();
    }

    pub fn is_empty(&self) -> bool {
        self.lines.iter().all(String::is_empty)
    }

    pub fn len_bytes(&self) -> usize {
        self.lines.iter().map(String::len).sum::<usize>() + self.lines.len().saturating_sub(1)
    }

    fn line_len(&self, idx: usize) -> usize {
        self.lines.get(idx).map_or(0, |l| graphemes(l).len())
    }

    fn clamp_cursor(&mut self) {
        self.cursor_line = self.cursor_line.min(self.lines.len().saturating_sub(1));
        self.cursor_col = self.cursor_col.min(self.line_len(self.cursor_line));
    }

    // ---- soạn thảo ----

    pub fn insert_char(&mut self, c: char) {
        if c == '\n' {
            self.insert_newline();
            return;
        }
        let at = byte_at(&self.lines[self.cursor_line], self.cursor_col);
        self.lines[self.cursor_line].insert(at, c);
        self.cursor_col += 1;
    }

    pub fn insert_newline(&mut self) {
        if self.single_line {
            return;
        }
        let at = byte_at(&self.lines[self.cursor_line], self.cursor_col);
        let rest = self.lines[self.cursor_line].split_off(at);
        self.lines.insert(self.cursor_line + 1, rest);
        self.cursor_line += 1;
        self.cursor_col = 0;
    }

    /// Bracketed paste — đường vào chính của dữ liệu, quan trọng hơn soạn thảo.
    pub fn insert_str(&mut self, text: &str) {
        if self.single_line {
            let flat = text.replace(['\n', '\r'], " ");
            let at = byte_at(&self.lines[self.cursor_line], self.cursor_col);
            self.lines[self.cursor_line].insert_str(at, &flat);
            self.cursor_col += graphemes(&flat).len();
            return;
        }

        let at = byte_at(&self.lines[self.cursor_line], self.cursor_col);
        let tail = self.lines[self.cursor_line].split_off(at);
        let mut parts = text.replace('\r', "");
        parts.push_str(&tail);

        let new_lines: Vec<String> = parts.split('\n').map(String::from).collect();
        let pasted_lines = text.replace('\r', "");
        let pasted_lines: Vec<&str> = pasted_lines.split('\n').collect();

        let head = self.lines[self.cursor_line].clone();
        self.lines.remove(self.cursor_line);
        for (i, l) in new_lines.iter().enumerate() {
            let content = if i == 0 {
                format!("{head}{l}")
            } else {
                l.clone()
            };
            self.lines.insert(self.cursor_line + i, content);
        }

        if pasted_lines.len() == 1 {
            self.cursor_col += graphemes(pasted_lines[0]).len();
        } else {
            self.cursor_line += pasted_lines.len() - 1;
            self.cursor_col = graphemes(pasted_lines[pasted_lines.len() - 1]).len();
        }
        self.clamp_cursor();
    }

    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            let start = byte_at(&self.lines[self.cursor_line], self.cursor_col - 1);
            let end = byte_at(&self.lines[self.cursor_line], self.cursor_col);
            self.lines[self.cursor_line].replace_range(start..end, "");
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            let cur = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            self.cursor_col = self.line_len(self.cursor_line);
            self.lines[self.cursor_line].push_str(&cur);
        }
    }

    pub fn delete(&mut self) {
        let len = self.line_len(self.cursor_line);
        if self.cursor_col < len {
            let start = byte_at(&self.lines[self.cursor_line], self.cursor_col);
            let end = byte_at(&self.lines[self.cursor_line], self.cursor_col + 1);
            self.lines[self.cursor_line].replace_range(start..end, "");
        } else if self.cursor_line + 1 < self.lines.len() {
            let next = self.lines.remove(self.cursor_line + 1);
            self.lines[self.cursor_line].push_str(&next);
        }
    }

    /// Ctrl+U — xóa từ đầu dòng tới con trỏ.
    pub fn delete_to_line_start(&mut self) {
        let end = byte_at(&self.lines[self.cursor_line], self.cursor_col);
        self.lines[self.cursor_line].replace_range(..end, "");
        self.cursor_col = 0;
    }

    // ---- di chuyển ----

    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.line_len(self.cursor_line);
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor_col < self.line_len(self.cursor_line) {
            self.cursor_col += 1;
        } else if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            self.cursor_col = 0;
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.clamp_cursor();
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            self.clamp_cursor();
        }
    }

    pub fn move_line_start(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_line_end(&mut self) {
        self.cursor_col = self.line_len(self.cursor_line);
    }

    // ---- soft wrap + scroll ----

    /// Cắt từng dòng logic thành các dòng hiển thị vừa `width` cột.
    fn visual_rows(&self, width: usize) -> Vec<VisualRow> {
        let width = width.max(1);
        let mut out = Vec::new();

        for (li, line) in self.lines.iter().enumerate() {
            let gs = graphemes(line);
            if gs.is_empty() {
                out.push((li, 0, 0));
                continue;
            }
            let mut start = 0;
            while start < gs.len() {
                let mut w = 0usize;
                let mut end = start;
                while end < gs.len() {
                    let gw = UnicodeWidthStr::width(gs[end]).max(1);
                    // Luôn nhận ít nhất một grapheme để vòng lặp chắc chắn tiến.
                    if w + gw > width && end > start {
                        break;
                    }
                    w += gw;
                    end += 1;
                }
                out.push((li, start, end));
                start = end;
            }
        }
        out
    }

    fn cursor_row(&self, rows: &[VisualRow]) -> usize {
        let mut last_of_line = 0;
        for (i, &(li, start, end)) in rows.iter().enumerate() {
            if li != self.cursor_line {
                continue;
            }
            last_of_line = i;
            if self.cursor_col >= start && self.cursor_col < end {
                return i;
            }
        }
        // Con trỏ ở cuối dòng: nằm trên dòng hiển thị cuối cùng của dòng logic đó.
        last_of_line
    }

    /// Nội dung để render cùng vị trí con trỏ `(cột, dòng)` tính theo ô hiển thị.
    pub fn render(&self, width: u16, height: u16) -> (Vec<String>, (u16, u16)) {
        let rows = self.visual_rows(width as usize);
        let cursor_row = self.cursor_row(&rows);
        let height = (height as usize).max(1);

        // Cuộn vừa đủ để con trỏ nằm trong khung.
        let mut scroll = self.scroll.get();
        if cursor_row < scroll {
            scroll = cursor_row;
        } else if cursor_row >= scroll + height {
            scroll = cursor_row + 1 - height;
        }
        scroll = scroll.min(rows.len().saturating_sub(1));
        self.scroll.set(scroll);

        let visible: Vec<String> = rows
            .iter()
            .skip(scroll)
            .take(height)
            .map(|&(li, start, end)| {
                let gs = graphemes(&self.lines[li]);
                gs[start..end].concat()
            })
            .collect();

        let (li, start, _) = rows[cursor_row];
        let gs = graphemes(&self.lines[li]);
        let col =
            UnicodeWidthStr::width(gs[start..self.cursor_col.min(gs.len())].concat().as_str());
        let cursor = (col as u16, (cursor_row.saturating_sub(scroll)) as u16);

        (visible, cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_moves_by_grapheme_not_byte() {
        let mut ta = TextArea::new(false);
        // "à" tổ hợp (a + U+0300) là một grapheme gồm nhiều byte.
        ta.set_value("a\u{0300}xin chào");
        ta.move_line_start();
        ta.move_right();
        // Sau một lần sang phải, con trỏ đã qua trọn cụm "à".
        assert_eq!(ta.cursor_col, 1);
        ta.backspace();
        assert_eq!(ta.value(), "xin chào");
    }

    #[test]
    fn emoji_is_one_grapheme() {
        let mut ta = TextArea::new(false);
        ta.set_value("👨‍👩‍👧‍👦!");
        ta.move_line_end();
        ta.backspace(); // xóa "!"
        ta.backspace(); // xóa trọn cụm emoji gia đình
        assert_eq!(ta.value(), "");
    }

    #[test]
    fn paste_multiline_lands_in_one_go() {
        let mut ta = TextArea::new(false);
        ta.set_value("start");
        ta.move_line_end();
        ta.insert_str("\nmiddle\nend");
        assert_eq!(ta.value(), "start\nmiddle\nend");
        assert_eq!(ta.cursor_line, 2);
        assert_eq!(ta.cursor_col, 3);
    }

    #[test]
    fn single_line_flattens_pasted_newlines() {
        let mut ta = TextArea::new(true);
        ta.insert_str("a\nb\nc");
        assert_eq!(ta.value(), "a b c");
        assert_eq!(ta.lines.len(), 1);
    }

    #[test]
    fn backspace_joins_lines() {
        let mut ta = TextArea::new(false);
        ta.set_value("ab\ncd");
        ta.cursor_line = 1;
        ta.cursor_col = 0;
        ta.backspace();
        assert_eq!(ta.value(), "abcd");
        assert_eq!(ta.cursor_col, 2);
    }

    #[test]
    fn delete_to_line_start_clears_prefix() {
        let mut ta = TextArea::new(true);
        ta.set_value("hello world");
        ta.move_line_end();
        ta.delete_to_line_start();
        assert_eq!(ta.value(), "");
    }

    #[test]
    fn soft_wrap_splits_long_line() {
        let mut ta = TextArea::new(false);
        ta.set_value("abcdefghij");
        let (rows, _) = ta.render(4, 10);
        assert_eq!(rows, vec!["abcd", "efgh", "ij"]);
    }

    #[test]
    fn wide_chars_respect_display_width() {
        let mut ta = TextArea::new(false);
        // Mỗi chữ Hán rộng 2 cột → chỉ 2 chữ vừa một dòng rộng 4.
        ta.set_value("漢字漢字");
        let (rows, _) = ta.render(4, 10);
        assert_eq!(rows, vec!["漢字", "漢字"]);
    }

    #[test]
    fn scroll_follows_cursor() {
        let mut ta = TextArea::new(false);
        ta.set_value("1\n2\n3\n4\n5");
        ta.cursor_line = 4;
        let (rows, cursor) = ta.render(10, 2);
        assert_eq!(rows, vec!["4", "5"]);
        assert_eq!(cursor.1, 1, "con trỏ phải nằm trong khung nhìn");
    }
}
