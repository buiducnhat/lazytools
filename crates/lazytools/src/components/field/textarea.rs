//! Hand-rolled TextArea — `tui-textarea 0.7` only supports `ratatui ^0.29`.
//!
//! **Scope is deliberately limited:** insert / delete / cursor movement / bracketed
//! paste / soft wrap + vertical scroll. **No** undo/redo, **no** selection,
//! **no** search. Users paste and look at the result rather than doing heavy editing
//! here; expanding the scope is the fastest way to make this phase slip its schedule.

use std::cell::Cell;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// A displayed row after soft wrap: `(logical line index, start grapheme, end grapheme)`.
type VisualRow = (usize, usize, usize);

#[derive(Debug, Clone)]
pub struct TextArea {
    lines: Vec<String>,
    /// Cursor is counted in **grapheme clusters**, not bytes.
    cursor_line: usize,
    cursor_col: usize,
    /// `Cell` so `render()` can update scroll while still keeping `draw(&self)`
    /// from `DrawableComponent`.
    scroll: Cell<usize>,
    single_line: bool,
}

fn graphemes(s: &str) -> Vec<&str> {
    s.graphemes(true).collect()
}

/// Byte position of the `n`th grapheme — used to slice the string without breaking UTF-8.
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

    // ---- editing ----

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

    /// Bracketed paste — the main data entry path, more important than editing.
    pub fn insert_str(&mut self, text: &str) {
        // Terminals deliver pasted line breaks as CR, CRLF, or LF depending on the emulator
        // and where the text was copied from. Normalize to LF first: stripping CR instead
        // silently glues adjacent lines into one, and CRLF would count as two breaks.
        let text = text.replace("\r\n", "\n").replace('\r', "\n");

        if self.single_line {
            let flat = text.replace('\n', " ");
            let at = byte_at(&self.lines[self.cursor_line], self.cursor_col);
            self.lines[self.cursor_line].insert_str(at, &flat);
            self.cursor_col += graphemes(&flat).len();
            return;
        }

        let at = byte_at(&self.lines[self.cursor_line], self.cursor_col);
        let tail = self.lines[self.cursor_line].split_off(at);
        let mut parts = text.clone();
        parts.push_str(&tail);

        let new_lines: Vec<String> = parts.split('\n').map(String::from).collect();
        let pasted_lines: Vec<&str> = text.split('\n').collect();

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

    /// Ctrl+U — delete from the start of the line to the cursor.
    pub fn delete_to_line_start(&mut self) {
        let end = byte_at(&self.lines[self.cursor_line], self.cursor_col);
        self.lines[self.cursor_line].replace_range(..end, "");
        self.cursor_col = 0;
    }

    // ---- movement ----

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

    /// Splits each logical line into displayed rows that fit `width` columns.
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
                    // Always take at least one grapheme so the loop is guaranteed to progress.
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
        // Cursor at the end of the line: sits on the last displayed row of that logical line.
        last_of_line
    }

    /// Content to render along with the cursor position `(col, row)` in display cells.
    pub fn render(&self, width: u16, height: u16) -> (Vec<String>, (u16, u16)) {
        let rows = self.visual_rows(width as usize);
        let cursor_row = self.cursor_row(&rows);
        let height = (height as usize).max(1);

        // Scroll just enough so the cursor stays within the viewport.
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
        // "à" (a + U+0300) is a combining grapheme made of multiple bytes.
        ta.set_value("a\u{0300}hello");
        ta.move_line_start();
        ta.move_right();
        // After moving right once, the cursor has passed the entire "à" cluster.
        assert_eq!(ta.cursor_col, 1);
        ta.backspace();
        assert_eq!(ta.value(), "hello");
    }

    #[test]
    fn emoji_is_one_grapheme() {
        let mut ta = TextArea::new(false);
        ta.set_value("👨‍👩‍👧‍👦!");
        ta.move_line_end();
        ta.backspace(); // delete "!"
        ta.backspace(); // delete the whole family emoji cluster
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

    /// Bracketed paste often arrives with CR line endings rather than LF. Stripping the CR
    /// used to glue every line into one — with trailing whitespace left behind, a pasted
    /// block looked like it had merely lost its line breaks to spaces.
    #[test]
    fn paste_treats_cr_line_endings_as_line_breaks() {
        let mut ta = TextArea::new(false);
        ta.insert_str("alpha\rbravo\rcharlie");
        assert_eq!(ta.value(), "alpha\nbravo\ncharlie");
        assert_eq!(ta.lines.len(), 3);
        assert_eq!(ta.cursor_line, 2);
    }

    #[test]
    fn paste_treats_crlf_as_one_line_break() {
        let mut ta = TextArea::new(false);
        ta.insert_str("alpha\r\nbravo\r\ncharlie");
        assert_eq!(ta.value(), "alpha\nbravo\ncharlie");
        assert_eq!(ta.lines.len(), 3);
    }

    #[test]
    fn single_line_flattens_pasted_newlines() {
        let mut ta = TextArea::new(true);
        ta.insert_str("a\nb\nc");
        assert_eq!(ta.value(), "a b c");
        assert_eq!(ta.lines.len(), 1);
    }

    /// A single-line field collapses each break to exactly one space, whichever encoding
    /// the terminal used — CRLF must not become two.
    #[test]
    fn single_line_flattens_cr_and_crlf_to_one_space_each() {
        let mut ta = TextArea::new(true);
        ta.insert_str("a\r\nb\rc");
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
        // Each Han character is 2 columns wide → only 2 characters fit on a 4-wide line.
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
        assert_eq!(cursor.1, 1, "cursor must stay within the viewport");
    }
}
