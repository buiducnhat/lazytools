use std::cell::RefCell;

use anyhow::Result;
use lazytools_core::registry::Registry;
use lazytools_core::spec::Category;
use ratatui::crossterm::event::{Event, MouseButton, MouseEventKind};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState};

use super::{CommandBlocking, CommandInfo, Component, DrawableComponent, EventState, LastArea};
use crate::keys::{KeyConfig, key_match};
use crate::queue::{InternalEvent, Queue};
use crate::ui::{inside, SharedTheme};

/// A flat list with group headers. A tool's `id`/`name` are both `&'static str`,
/// so the sidebar doesn't need to borrow `Registry` after construction.
enum Row {
    Header(&'static str),
    Tool {
        id: &'static str,
        name: &'static str,
    },
}

/// First character — sliced by `char`, not by byte, so it doesn't panic on UTF-8.
fn first_char(s: &str) -> String {
    s.chars().next().map(String::from).unwrap_or_default()
}

pub struct Sidebar {
    rows: Vec<Row>,
    selected: usize,
    /// The scroll offset has to **survive between frames**. Ratatui only ever nudges the
    /// offset far enough to bring the selection into view, so a state rebuilt at offset 0
    /// on every draw pins the selection to the bottom row the moment the catalog is
    /// taller than the pane — and it stays pinned all the way back up. `RefCell` because
    /// `DrawableComponent::draw` takes `&self`.
    list_state: RefCell<ListState>,
    focused: bool,
    queue: Queue,
    theme: SharedTheme,
    key_config: KeyConfig,
    /// Published for the App-level outside-click guard. Written at the start of `draw`
    /// so the event handler can compare a click position to the pane's last rect.
    last_area: LastArea,
}

impl Sidebar {
    pub fn new(
        registry: &Registry,
        queue: Queue,
        theme: SharedTheme,
        key_config: KeyConfig,
    ) -> Self {
        let mut rows = Vec::new();
        for &category in Category::ALL {
            let mut tools = registry.by_category(category).peekable();
            if tools.peek().is_none() {
                continue;
            }
            rows.push(Row::Header(category.label()));
            for tool in tools {
                rows.push(Row::Tool {
                    id: tool.spec().id,
                    name: tool.spec().name,
                });
            }
        }

        let selected = rows
            .iter()
            .position(|r| matches!(r, Row::Tool { .. }))
            .unwrap_or(0);

        Self {
            rows,
            selected,
            list_state: RefCell::new(ListState::default()),
            focused: true,
            queue,
            theme,
            key_config,
            last_area: LastArea::default(),
        }
    }

    /// The currently selected tool — `App` uses this to load the form at startup.
    pub fn selected_tool(&self) -> Option<&'static str> {
        match self.rows.get(self.selected) {
            Some(Row::Tool { id, .. }) => Some(id),
            _ => None,
        }
    }

    /// Moves the highlight onto `id`. Returns `false` if no such tool is listed.
    ///
    /// Deliberately does **not** queue `SelectTool`: this is what `App` calls
    /// *while handling* one, so that a tool picked from the palette — or
    /// restored from the last session — leaves the sidebar pointing at the tool
    /// actually on screen rather than at whatever was there before.
    pub fn select_tool(&mut self, id: &str) -> bool {
        match self
            .rows
            .iter()
            .position(|r| matches!(r, Row::Tool { id: row_id, .. } if *row_id == id))
        {
            Some(idx) => {
                self.selected = idx;
                true
            }
            None => false,
        }
    }

    /// Moves to the next tool row in the direction of `delta`, skipping group headers.
    fn move_selection(&mut self, delta: isize) {
        let mut idx = self.selected as isize;
        loop {
            idx += delta;
            if idx < 0 || idx as usize >= self.rows.len() {
                return;
            }
            if matches!(self.rows[idx as usize], Row::Tool { .. }) {
                break;
            }
        }
        self.selected = idx as usize;
        if let Some(id) = self.selected_tool() {
            self.queue.push(InternalEvent::SelectTool(id));
        }
    }
}

impl DrawableComponent for Sidebar {
    fn draw(&self, f: &mut Frame, rect: Rect) -> Result<()> {
        // Below 60 cols the sidebar is hidden entirely, `App` passes an empty rect.
        self.last_area.set(rect);
        if rect.width == 0 {
            return Ok(());
        }
        let icon_only = rect.width < 12;

        let items: Vec<ListItem> = self
            .rows
            .iter()
            .map(|row| match row {
                Row::Header(name) => {
                    let text = if icon_only {
                        first_char(name)
                    } else {
                        (*name).to_string()
                    };
                    ListItem::new(Line::from(Span::styled(text, self.theme.group())))
                }
                Row::Tool { name, .. } => {
                    let text = if icon_only {
                        format!(" {}", first_char(name))
                    } else {
                        format!("  {name}")
                    };
                    ListItem::new(Line::from(Span::styled(text, self.theme.text())))
                }
            })
            .collect();

        let block = Block::bordered()
            .border_style(self.theme.block(self.focused))
            .title_style(self.theme.title(self.focused))
            .title(if icon_only { "" } else { " Tools " });

        let list = List::new(items)
            .block(block)
            .highlight_style(self.theme.selection());

        let mut state = self.list_state.borrow_mut();
        state.select(Some(self.selected));
        f.render_stateful_widget(list, rect, &mut state);
        Ok(())
    }
}

impl Component for Sidebar {
    fn commands(&self, out: &mut Vec<CommandInfo>, force_all: bool) -> CommandBlocking {
        if self.focused || force_all {
            let keys = &self.key_config.keys;
            out.push(
                CommandInfo::new(
                    format!(
                        "{}/{}",
                        self.key_config.hint(keys.move_down),
                        self.key_config.hint(keys.move_up)
                    ),
                    "select tool",
                    "Sidebar",
                )
                .order(1),
            );
        }
        CommandBlocking::PassingOn
    }

    fn event(&mut self, ev: &Event) -> Result<EventState> {
        // Mouse events bypass the focus gate: a click on the sidebar
        // always responds, so the user can switch panes with the mouse
        // even when the form currently holds focus.
        if !self.focused
            && !matches!(ev, Event::Mouse(_))
        {
            return Ok(EventState::NotConsumed);
        }

        if let Event::Mouse(m) = ev {
            let rect = self.last_area.get();
            // A click outside the sidebar's pane falls through to the outside-click guard.
            if !inside(rect, m.column, m.row) {
                return Ok(EventState::NotConsumed);
            }
            let block = Block::bordered(); // border is 1
            let inner = block.inner(rect);
            let Event::Mouse(m) = ev else { unreachable!() };
            match m.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    let offset = self.list_state.borrow().offset();
                    // `inner.y` is already `rect.y + 1` due to the border.
                    // Don't add another 1 — the first list row is at `inner.y`.
                    let row = m.row.saturating_sub(inner.y);
                    let idx = offset + row as usize;
                    if let Some(Row::Tool { id, .. }) = self.rows.get(idx) {
                        self.selected = idx;
                        self.queue.push(InternalEvent::SelectTool(id));
                        self.queue.push(InternalEvent::FocusPane(crate::app::Focus::Sidebar));
                    }
                }
                MouseEventKind::ScrollUp => self.move_selection(-1),
                MouseEventKind::ScrollDown => self.move_selection(1),
                _ => {}
            }
            return Ok(EventState::Consumed);
        }

        let Event::Key(k) = ev else {
            return Ok(EventState::NotConsumed);
        };
        let keys = &self.key_config.keys;

        if key_match(k, keys.move_down) || key_match(k, keys.move_down_alt) {
            self.move_selection(1);
            return Ok(EventState::Consumed);
        }
        if key_match(k, keys.move_up) || key_match(k, keys.move_up_alt) {
            self.move_selection(-1);
            return Ok(EventState::Consumed);
        }
        if key_match(k, keys.confirm) {
            if let Some(id) = self.selected_tool() {
                self.queue.push(InternalEvent::SelectTool(id));
            }
            return Ok(EventState::Consumed);
        }
        Ok(EventState::NotConsumed)
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}
