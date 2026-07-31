//! Fuzzy finder mở bằng `Ctrl+P`. Khớp trên `name + keywords + description`.
//!
//! Dùng `nucleo::pattern::Pattern` (một lần chấm điểm) thay vì `nucleo::Nucleo`
//! (worker đa luồng, tăng dần): catalog chỉ vài chục tool và nằm sẵn trong bộ
//! nhớ, nên máy chấm điểm là đủ còn tầng worker chỉ là chi phí thừa.

use anyhow::Result;
use lazytools_core::registry::Registry;
use nucleo::Matcher;
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph};

use super::field::textarea::TextArea;
use super::{CommandBlocking, CommandInfo, Component, DrawableComponent, EventState};
use crate::keys::{KeyConfig, key_match, typed_char};
use crate::queue::{InternalEvent, Queue};
use crate::ui::{SharedTheme, centered_rect};

/// Một mục có thể chọn — `haystack` gộp sẵn để không phải nối chuỗi mỗi lần gõ.
struct Entry {
    id: &'static str,
    name: &'static str,
    category: &'static str,
    haystack: String,
}

pub struct Palette {
    entries: Vec<Entry>,
    /// Chỉ số vào `entries`, đã lọc và sắp theo điểm.
    filtered: Vec<usize>,
    selected: usize,
    input: TextArea,
    visible: bool,
    matcher: std::cell::RefCell<Matcher>,
    queue: Queue,
    theme: SharedTheme,
    key_config: KeyConfig,
}

impl Palette {
    pub fn new(
        registry: &Registry,
        queue: Queue,
        theme: SharedTheme,
        key_config: KeyConfig,
    ) -> Self {
        let entries = registry
            .all()
            .map(|tool| {
                let spec = tool.spec();
                Entry {
                    id: spec.id,
                    name: spec.name,
                    category: spec.category.label(),
                    haystack: format!(
                        "{} {} {} {}",
                        spec.name,
                        spec.keywords.join(" "),
                        spec.description,
                        spec.cli_name()
                    ),
                }
            })
            .collect::<Vec<_>>();

        let mut palette = Self {
            filtered: (0..entries.len()).collect(),
            entries,
            selected: 0,
            input: TextArea::new(true),
            visible: false,
            matcher: std::cell::RefCell::new(Matcher::default()),
            queue,
            theme,
            key_config,
        };
        palette.refilter();
        palette
    }

    fn refilter(&mut self) {
        let needle = self.input.value();
        if needle.is_empty() {
            self.filtered = (0..self.entries.len()).collect();
            self.selected = 0;
            return;
        }

        let pattern = Pattern::parse(&needle, CaseMatching::Ignore, Normalization::Smart);
        let mut matcher = self.matcher.borrow_mut();
        let mut buf = Vec::new();

        let mut scored: Vec<(u32, usize)> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                let haystack = nucleo::Utf32Str::new(&e.haystack, &mut buf);
                pattern.score(haystack, &mut matcher).map(|s| (s, i))
            })
            .collect();

        // Điểm cao lên trước; hòa điểm thì giữ thứ tự registry cho ổn định.
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        self.filtered = scored.into_iter().map(|(_, i)| i).collect();
        self.selected = 0;
    }

    fn selected_id(&self) -> Option<&'static str> {
        self.filtered
            .get(self.selected)
            .map(|&i| self.entries[i].id)
    }

    fn move_selection(&mut self, delta: isize) {
        if self.filtered.is_empty() {
            return;
        }
        let len = self.filtered.len() as isize;
        self.selected = ((self.selected as isize + delta).rem_euclid(len)) as usize;
    }
}

impl DrawableComponent for Palette {
    fn draw(&self, f: &mut Frame, rect: Rect) -> Result<()> {
        if !self.visible {
            return Ok(());
        }
        let area = centered_rect(70, 60, rect);
        f.render_widget(Clear, area);

        let block = Block::bordered()
            .border_style(self.theme.block(true))
            .title_style(self.theme.title(true))
            .title(" Tìm tool ");
        let inner = block.inner(area);
        f.render_widget(block, area);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(inner);

        let needle = self.input.value();
        f.render_widget(
            Paragraph::new(format!("> {needle}")).style(self.theme.text()),
            rows[0],
        );
        f.set_cursor_position((rows[0].x + 2 + needle.chars().count() as u16, rows[0].y));

        let items: Vec<ListItem> = self
            .filtered
            .iter()
            .map(|&i| {
                let e = &self.entries[i];
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{:<16}", e.name), self.theme.text()),
                    Span::styled(e.category, self.theme.dim()),
                ]))
            })
            .collect();

        let mut state = ListState::default();
        if !self.filtered.is_empty() {
            state.select(Some(self.selected));
        }
        f.render_stateful_widget(
            List::new(items).highlight_style(self.theme.selection()),
            rows[1],
            &mut state,
        );
        Ok(())
    }
}

impl Component for Palette {
    fn commands(&self, out: &mut Vec<CommandInfo>, force_all: bool) -> CommandBlocking {
        // Lệnh *mở* palette là affordance cấp app (`App::app_commands`);
        // ở đây chỉ công bố các lệnh dùng được khi palette đang mở.
        if self.visible || force_all {
            let keys = &self.key_config.keys;
            out.push(
                CommandInfo::new(
                    format!(
                        "{}/{}",
                        self.key_config.hint(keys.move_down_alt),
                        self.key_config.hint(keys.move_up_alt)
                    ),
                    "chọn",
                    "Palette",
                )
                .order(10),
            );
            out.push(
                CommandInfo::new(self.key_config.hint(keys.confirm), "mở tool", "Palette")
                    .order(11),
            );
        }
        if self.visible {
            CommandBlocking::Blocking
        } else {
            CommandBlocking::PassingOn
        }
    }

    fn event(&mut self, ev: &Event) -> Result<EventState> {
        if !self.visible {
            return Ok(EventState::NotConsumed);
        }

        if let Event::Paste(text) = ev {
            self.input.insert_str(text);
            self.refilter();
            return Ok(EventState::Consumed);
        }

        let Event::Key(k) = ev else {
            return Ok(EventState::Consumed);
        };
        let b = &self.key_config.keys;

        if key_match(k, b.exit_popup) {
            self.queue.push(InternalEvent::ClosePalette);
        } else if key_match(k, b.confirm) {
            if let Some(id) = self.selected_id() {
                self.queue.push(InternalEvent::SelectTool(id));
            }
            self.queue.push(InternalEvent::ClosePalette);
        } else if key_match(k, b.move_down_alt) {
            self.move_selection(1);
        } else if key_match(k, b.move_up_alt) {
            self.move_selection(-1);
        } else if key_match(k, b.backspace) {
            self.input.backspace();
            self.refilter();
        } else if key_match(k, b.delete_to_start) {
            self.input.delete_to_line_start();
            self.refilter();
        } else if let Some(c) = typed_char(k) {
            self.input.insert_char(c);
            self.refilter();
        }
        // Palette đang mở thì nuốt mọi phím.
        Ok(EventState::Consumed)
    }

    fn focused(&self) -> bool {
        self.visible
    }

    fn set_focused(&mut self, _focused: bool) {}

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn hide(&mut self) {
        self.visible = false;
    }

    fn show(&mut self) -> Result<()> {
        self.visible = true;
        self.input.set_value("");
        self.refilter();
        Ok(())
    }
}
