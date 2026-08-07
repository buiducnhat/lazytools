//! Fuzzy finder opened with `Ctrl+P`. Matches on `name + keywords + description`.
//!
//! Uses `nucleo::pattern::Pattern` (single-shot scoring) instead of `nucleo::Nucleo`
//! (multi-threaded, incremental worker): the catalog is only a few dozen tools and
//! already sits in memory, so the plain scorer is enough and the worker layer would
//! just be overhead.

use anyhow::Result;
use lazytools_core::registry::Registry;
use nucleo::Matcher;
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use ratatui::crossterm::event::{Event, MouseButton, MouseEventKind};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph};

use super::field::textarea::TextArea;
use super::{CommandBlocking, CommandInfo, Component, DrawableComponent, EventState, LastArea};
use crate::keys::{KeyConfig, key_match, typed_char};
use crate::queue::{InternalEvent, Queue};
use crate::ui::{inside, SharedTheme, centered_rect};

/// A selectable entry — `haystack` is pre-joined so we don't concatenate strings on every keystroke.
struct Entry {
    id: &'static str,
    name: &'static str,
    category: &'static str,
    haystack: String,
}

pub struct Palette {
    entries: Vec<Entry>,
    /// Indices into `entries`, filtered and sorted by score.
    filtered: Vec<usize>,
    selected: usize,
    input: TextArea,
    visible: bool,
    matcher: std::cell::RefCell<Matcher>,
    queue: Queue,
    theme: SharedTheme,
    key_config: KeyConfig,
    /// Published for the App-level outside-click guard.
    last_area: LastArea,
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
            last_area: LastArea::default(),
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

        // Highest score first; ties keep registry order for stability.
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
        self.last_area.set(area);
        f.render_widget(Clear, area);

        let block = Block::bordered()
            .style(self.theme.base())
            .border_style(self.theme.block(true))
            .title_style(self.theme.title(true))
            .title(" Find tool ");
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
        // The command to *open* the palette is an app-level affordance (`App::app_commands`);
        // here we only advertise the commands usable while the palette is open.
        if self.visible || force_all {
            let keys = &self.key_config.keys;
            out.push(
                CommandInfo::new(
                    format!(
                        "{}/{}",
                        self.key_config.hint(keys.move_down_alt),
                        self.key_config.hint(keys.move_up_alt)
                    ),
                    "select",
                    "Palette",
                )
                .order(10),
            );
            out.push(
                CommandInfo::new(self.key_config.hint(keys.confirm), "open tool", "Palette")
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

        if let Event::Mouse(m) = ev {
            let area = self.last_area.get();
            // Click outside the popup: close it.
            if !inside(area, m.column, m.row) {
                self.queue.push(InternalEvent::ClosePalette);
                return Ok(EventState::Consumed);
            }
            // Recompute the inner layout exactly as `draw` does.
            let block = Block::bordered();
            let inner = block.inner(area);
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(1)])
                .split(inner);
            let Event::Mouse(m) = ev else { unreachable!() };
            match m.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    if m.row == rows[0].y {
                        // Click on the search input — just focus it (cursor is always at the end).
                    } else if m.row >= rows[1].y && m.row < rows[1].y + rows[1].height {
                        // Click on a list row.
                        let idx = (m.row - rows[1].y) as usize;
                        if idx < self.filtered.len() {
                            self.selected = idx;
                            if let Some(id) = self.selected_id() {
                                self.queue.push(InternalEvent::SelectTool(id));
                            }
                            self.queue.push(InternalEvent::ClosePalette);
                        }
                    }
                }
                MouseEventKind::ScrollUp => self.move_selection(-1),
                MouseEventKind::ScrollDown => self.move_selection(1),
                _ => {}
            }
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
        // While the palette is open, it swallows every keypress.
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
