//! The theme picker, opened with `Ctrl+T`.
//!
//! It previews as it moves: every keystroke that changes the selection re-themes
//! the *whole app* behind the popup, because a swatch is a poor substitute for
//! seeing your own tool in the colors you are choosing. `Esc` puts back the
//! theme that was in force when it opened; `Enter` keeps the new one and saves
//! the choice.
//!
//! The popup neither resolves nor stores a theme itself. It emits intent —
//! preview this one, apply this one — and `App` answers, because the theme that
//! results also depends on the per-color overrides in `config.toml`, which are
//! settings and not the picker's business.

use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph};

use crate::components::{CommandBlocking, CommandInfo, Component, DrawableComponent, EventState};
use crate::keys::{KeyConfig, key_match};
use crate::queue::{InternalEvent, Queue};
use crate::ui::{SharedTheme, centered_rect, themes};

pub struct ThemePopup {
    selected: usize,
    /// What to go back to if the picker is dismissed — the theme that was
    /// active when it opened, not the configured default: those differ the
    /// moment someone opens the picker twice.
    opened_with: usize,
    visible: bool,
    queue: Queue,
    theme: SharedTheme,
    key_config: KeyConfig,
}

impl ThemePopup {
    pub fn new(queue: Queue, theme: SharedTheme, key_config: KeyConfig) -> Self {
        Self {
            selected: 0,
            opened_with: 0,
            visible: false,
            queue,
            theme,
            key_config,
        }
    }

    /// Opens with the cursor on the theme currently in use.
    pub fn show_with(&mut self, active_id: &str) -> Result<()> {
        self.selected = themes::index_of(active_id).unwrap_or(0);
        self.opened_with = self.selected;
        self.visible = true;
        Ok(())
    }

    fn move_selection(&mut self, delta: isize) {
        let len = themes::PRESETS.len() as isize;
        self.selected = ((self.selected as isize + delta).rem_euclid(len)) as usize;
        self.preview(self.selected);
    }

    fn preview(&self, index: usize) {
        self.queue
            .push(InternalEvent::PreviewTheme(themes::PRESETS[index].id));
    }
}

impl DrawableComponent for ThemePopup {
    fn draw(&self, f: &mut Frame, rect: Rect) -> Result<()> {
        if !self.visible {
            return Ok(());
        }
        let area = centered_rect(50, 60, rect);
        f.render_widget(Clear, area);

        let block = Block::bordered()
            .style(self.theme.base())
            .border_style(self.theme.block(true))
            .title_style(self.theme.title(true))
            .title(" Theme ");
        let inner = block.inner(area);
        f.render_widget(block, area);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner);

        let items: Vec<ListItem> = themes::PRESETS
            .iter()
            .enumerate()
            .map(|(i, preset)| {
                let t = &preset.theme;
                // The swatch is drawn in the preset's *own* colors, so the list
                // stays readable as a comparison even while the app behind it
                // shows only the one being previewed.
                let mut spans = vec![
                    Span::raw(if i == self.opened_with { "● " } else { "  " }),
                    Span::raw(format!("{:<20}", preset.name)),
                ];
                for color in [t.title, t.border_focus, t.text, t.text_dim, t.error] {
                    spans.push(Span::styled("██", Style::default().fg(color)));
                }
                ListItem::new(Line::from(spans))
            })
            .collect();

        let mut state = ListState::default();
        state.select(Some(self.selected));
        f.render_stateful_widget(
            List::new(items).highlight_style(self.theme.selection()),
            rows[0],
            &mut state,
        );

        let keys = &self.key_config.keys;
        f.render_widget(
            Paragraph::new(format!(
                "{}/{} preview · {} keep · {} cancel",
                self.key_config.hint(keys.move_up_alt),
                self.key_config.hint(keys.move_down_alt),
                self.key_config.hint(keys.confirm),
                self.key_config.hint(keys.exit_popup),
            ))
            .style(self.theme.dim()),
            rows[1],
        );
        Ok(())
    }
}

impl Component for ThemePopup {
    fn commands(&self, out: &mut Vec<CommandInfo>, force_all: bool) -> CommandBlocking {
        // Opening it is an app-level affordance, like the palette; only what
        // works *while it is open* is declared here.
        if self.visible || force_all {
            let keys = &self.key_config.keys;
            out.push(
                CommandInfo::new(
                    format!(
                        "{}/{}",
                        self.key_config.hint(keys.move_down_alt),
                        self.key_config.hint(keys.move_up_alt)
                    ),
                    "preview theme",
                    "Theme",
                )
                .order(10),
            );
            out.push(
                CommandInfo::new(self.key_config.hint(keys.confirm), "keep theme", "Theme")
                    .order(11),
            );
            out.push(
                CommandInfo::new(self.key_config.hint(keys.exit_popup), "cancel", "Theme")
                    .order(12),
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
        let Event::Key(k) = ev else {
            return Ok(EventState::Consumed);
        };
        let b = &self.key_config.keys;

        if key_match(k, b.exit_popup) {
            // Undo the previews before closing — a cancelled picker must leave
            // no trace of what was tried.
            self.preview(self.opened_with);
            self.visible = false;
        } else if key_match(k, b.confirm) {
            self.queue
                .push(InternalEvent::ApplyTheme(themes::PRESETS[self.selected].id));
            self.visible = false;
        } else if key_match(k, b.move_down_alt) || key_match(k, b.move_down) {
            self.move_selection(1);
        } else if key_match(k, b.move_up_alt) || key_match(k, b.move_up) {
            self.move_selection(-1);
        }
        // While it is open it swallows every key, the palette's rule.
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
        self.show_with(themes::DEFAULT_ID)
    }
}
