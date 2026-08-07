use std::rc::Rc;

use anyhow::Result;
use lazytools_core::registry::{Registry, Tool};
use ratatui::Frame;
use ratatui::crossterm::event::{Event, MouseButton, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::Block;

use crate::clipboard;
use crate::components::cmdbar::CommandBar;
use crate::components::palette::Palette;
use crate::components::sidebar::Sidebar;
use crate::components::tool_form::ToolFormComponent;
use crate::components::{CommandInfo, Component, DrawableComponent, command_pump, event_pump};
use crate::keys::{KeyConfig, key_match};
use crate::popups::{FileOpenPopup, FileSavePopup, HelpPopup, MsgPopup, ThemePopup};
use crate::queue::{InternalEvent, NeedsUpdate, Queue};
use crate::session::{self, Session};
use crate::settings::Settings;
use crate::theme_state::{self, ThemeState};
use crate::ui::{SharedTheme, ThemeHandle, themes};

/// Responsive breakpoints: below 80 cols the sidebar shrinks to icons; below 60 it's hidden entirely.
const SIDEBAR_WIDTH: u16 = 24;
const SIDEBAR_WIDTH_NARROW: u16 = 6;
const BREAKPOINT_NARROW: u16 = 80;
const BREAKPOINT_HIDE: u16 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Workspace,
}

pub struct App {
    registry: Registry,
    queue: Queue,
    theme: SharedTheme,
    key_config: KeyConfig,
    settings: Settings,
    sidebar: Sidebar,
    tool_form: ToolFormComponent,
    palette: Palette,
    cmdbar: CommandBar,
    msg_popup: MsgPopup,
    help_popup: HelpPopup,
    file_open: FileOpenPopup,
    file_save: FileSavePopup,
    theme_popup: ThemePopup,
    /// Id of the preset in use. The colors themselves live in `theme`; this is
    /// what gets written down, and what the picker opens its cursor on.
    theme_id: String,
    /// Whether the picker was used in this run. Without it, quitting would
    /// write a `theme.toml` for someone who never asked for one — and that
    /// file would then start shadowing later edits to `config.toml`.
    theme_picked: bool,
    focus: Focus,
    should_quit: bool,
    needs_redraw: bool,
    /// Transient message in the cmdbar (e.g. "copied").
    flash: Option<String>,
}

impl App {
    /// Defaults throughout, and no session — for tests and for cases where
    /// there's no HOME.
    pub fn new(registry: Registry) -> Self {
        Self::build(
            registry,
            KeyConfig::default(),
            Settings::default(),
            Session::default(),
            themes::DEFAULT_ID.to_string(),
            Vec::new(),
        )
    }

    /// Reads `keys.toml`, `config.toml`, and the saved session. A broken config
    /// does **not** block startup: the app still opens, with a popup explaining
    /// the issue so the user can go fix it.
    pub fn from_user_config(registry: Registry) -> Self {
        let (key_config, key_issue) = KeyConfig::load();
        let (mut settings, settings_issue) = Settings::load();

        // The theme picked in a previous run, unless `config.toml` has been
        // edited since — see `theme_state` for why that is decidable.
        let theme_id =
            theme_state::resolve(settings.theme_name.as_deref(), ThemeState::load().as_ref());
        settings.theme = settings.theme_for(&theme_id);

        // Reading a session the user has turned off would be a promise broken
        // in the direction that matters.
        let session = if settings.restore.is_off() {
            Session::default()
        } else {
            Session::load()
        };
        let issues = [
            key_issue.map(|i| i.message()),
            settings_issue.map(|i| i.message()),
        ]
        .into_iter()
        .flatten()
        .collect();
        Self::build(registry, key_config, settings, session, theme_id, issues)
    }

    /// Starts from an explicit session and settings instead of reading the
    /// user's files — the seam the persistence tests drive, and the one a host
    /// embedding the app would use.
    pub fn with_settings(registry: Registry, settings: Settings, session: Session) -> Self {
        let theme_id = settings
            .theme_name
            .clone()
            .unwrap_or_else(|| themes::DEFAULT_ID.to_string());
        Self::build(
            registry,
            KeyConfig::default(),
            settings,
            session,
            theme_id,
            Vec::new(),
        )
    }

    fn build(
        registry: Registry,
        key_config: KeyConfig,
        settings: Settings,
        session: Session,
        theme_id: String,
        config_issues: Vec<String>,
    ) -> Self {
        let queue = Queue::default();
        // One handle, cloned into every component. It holds the colors behind a
        // `Cell` so the picker can swap them mid-run: a component handed a copy
        // of the theme would keep drawing the old one.
        let theme: SharedTheme = Rc::new(ThemeHandle::new(settings.theme));

        let mut sidebar = Sidebar::new(&registry, queue.clone(), theme.clone(), key_config);
        let mut tool_form = ToolFormComponent::new(queue.clone(), theme.clone(), key_config);

        // A tool that has since been removed from the catalog leaves the sidebar
        // on its default selection rather than opening nothing.
        let restored = session
            .tool
            .as_deref()
            .filter(|id| sidebar.select_tool(id))
            .is_some();

        // Open the first tool by default so the initial screen isn't empty.
        if let Some(tool) = sidebar.selected_tool().and_then(|id| registry.get(id)) {
            tool_form.set_tool(tool.spec());
            if restored {
                // After `set_tool`, which has just loaded every field's default.
                for (key, value) in
                    session::restorable(tool.spec(), &session.values, settings.restore)
                {
                    tool_form.set_field_value(key, &value);
                }
            }
        }

        let palette = Palette::new(&registry, queue.clone(), theme.clone(), key_config);
        let mut msg_popup = MsgPopup::new(theme.clone(), key_config);
        if !config_issues.is_empty() {
            msg_popup.show_error(config_issues.join("\n\n"));
        }

        let file_open = FileOpenPopup::new(queue.clone(), theme.clone(), key_config);
        let file_save = FileSavePopup::new(queue.clone(), theme.clone(), key_config);
        let theme_popup = ThemePopup::new(queue.clone(), theme.clone(), key_config);

        Self {
            registry,
            queue,
            key_config,
            settings,
            sidebar,
            tool_form,
            palette,
            cmdbar: CommandBar::new(theme.clone()),
            msg_popup,
            help_popup: HelpPopup::new(theme.clone(), key_config),
            file_open,
            file_save,
            theme_popup,
            theme_id,
            theme_picked: false,
            theme,
            focus: Focus::Sidebar,
            should_quit: false,
            needs_redraw: true,
            flash: None,
        }
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    fn sidebar_width(area_width: u16) -> u16 {
        if area_width < BREAKPOINT_HIDE {
            0
        } else if area_width < BREAKPOINT_NARROW {
            SIDEBAR_WIDTH_NARROW
        } else {
            SIDEBAR_WIDTH
        }
    }

    pub fn draw(&mut self, f: &mut Frame) -> Result<()> {
        self.needs_redraw = false;
        let area = f.area();

        // The surface first: a theme naming its own background has to paint it
        // before anything else draws on top. The default theme's `Reset` makes
        // this a no-op, which is why nothing else has to know about it.
        f.render_widget(Block::default().style(self.theme.base()), area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);
        let (main, cmdbar_area) = (chunks[0], chunks[1]);

        let sidebar_width = Self::sidebar_width(main.width);
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(sidebar_width), Constraint::Min(1)])
            .split(main);

        self.sidebar.draw(f, cols[0])?;
        self.draw_workspace(f, cols[1])?;

        self.refresh_commands();
        match &self.flash {
            Some(msg) => f.render_widget(
                ratatui::widgets::Paragraph::new(msg.as_str()).style(self.theme.title(true)),
                cmdbar_area,
            ),
            None => self.cmdbar.draw(f, cmdbar_area)?,
        }

        // Overlays draw last so they sit on top of everything.
        self.palette.draw(f, area)?;
        self.help_popup.draw(f, area)?;
        self.file_open.draw(f, area)?;
        self.file_save.draw(f, area)?;
        self.theme_popup.draw(f, area)?;
        self.msg_popup.draw(f, area)?;
        Ok(())
    }

    /// Frame of the currently open tool; the interior is built by `ToolFormComponent` from the `ToolSpec`.
    fn draw_workspace(&self, f: &mut Frame, rect: Rect) -> Result<()> {
        let focused = self.focus == Focus::Workspace;
        let name = self
            .tool_form
            .tool_id()
            .and_then(|id| self.registry.get(id))
            .map(|t| t.spec().name)
            .unwrap_or("—");

        let block = Block::bordered()
            .style(self.theme.base())
            .border_style(self.theme.block(focused))
            .title_style(self.theme.title(focused))
            .title(format!(" {name} "));

        let inner = block.inner(rect);
        f.render_widget(block, rect);
        self.tool_form.draw(f, inner)
    }

    fn components(&self) -> Vec<&dyn Component> {
        vec![
            &self.msg_popup,
            &self.help_popup,
            &self.file_open,
            &self.file_save,
            &self.theme_popup,
            &self.palette,
            &self.sidebar,
            &self.tool_form,
        ]
    }

    fn refresh_commands(&mut self) {
        let mut cmds = Vec::new();
        command_pump(&mut cmds, false, &self.components());
        cmds.extend(self.app_commands(false));
        self.cmdbar.set_cmds(cmds);
    }

    /// All commands, including those from hidden components — content for the help popup.
    fn all_commands(&self) -> Vec<CommandInfo> {
        let mut cmds = Vec::new();
        command_pump(&mut cmds, true, &self.components());
        cmds.extend(self.app_commands(true));
        cmds
    }

    /// App-level commands — always takes the key string from `KeyConfig`.
    ///
    /// `force_all` mirrors `Component::commands`: the one-line command bar shows only
    /// what the current focus can actually do, while the help popup lists everything.
    /// A key that is missing from the help screen is a key nobody finds.
    fn app_commands(&self, force_all: bool) -> Vec<CommandInfo> {
        let keys = &self.key_config.keys;
        let in_form = force_all || self.focus == Focus::Workspace;
        let mut cmds = Vec::new();
        // In the form, `Tab` is already advertised as "next field" — don't show a duplicate key.
        if force_all || self.focus == Focus::Sidebar {
            cmds.push(
                CommandInfo::new(self.key_config.hint(keys.focus_next), "switch pane", "App")
                    .order(50),
            );
        }
        // Only from the form: on the sidebar it would already be where it takes you.
        if in_form {
            cmds.push(
                CommandInfo::new(self.key_config.hint(keys.focus_sidebar), "tools", "App")
                    .order(51),
            );
        }
        cmds.push(CommandInfo::new(self.key_config.hint(keys.palette), "palette", "App").order(55));
        cmds.push(CommandInfo::new(self.key_config.hint(keys.theme), "theme", "App").order(59));
        if in_form {
            cmds.push(CommandInfo::new(self.key_config.hint(keys.copy), "copy", "App").order(58));
        }
        // Deliberately *not* under `force_all`, unlike the focus checks around it. The
        // difference is real: `copy` and `tools` work fine, you just have to be in the
        // right pane — help should name them. `open file` does nothing at all for a tool
        // with no input, and a help screen listing a key that only ever answers "this
        // tool has no input" is worse than one that omits it.
        if self.tool_form.accepts_file_input() {
            cmds.push(
                CommandInfo::new(self.key_config.hint(keys.open_file), "open file", "App")
                    .order(56),
            );
        }
        if in_form {
            cmds.push(
                CommandInfo::new(self.key_config.hint(keys.save_file), "save file", "App")
                    .order(57),
            );
        }
        cmds.push(CommandInfo::new(self.key_config.hint(keys.help), "help", "App").order(60));
        cmds.push(CommandInfo::new(self.key_config.hint(keys.quit), "quit", "App").order(99));
        cmds
    }

    pub fn event(&mut self, ev: &Event) -> Result<()> {
        self.needs_redraw = true;
        // The transient message disappears on the next action.
        self.flash = None;

        // Routing order: popups → palette → the focused pane.
        // Every component returns `NotConsumed` on its own when not focused/visible.
        let mut components: Vec<&mut dyn Component> = vec![
            &mut self.msg_popup,
            &mut self.help_popup,
            &mut self.file_open,
            &mut self.file_save,
            &mut self.theme_popup,
            &mut self.palette,
            &mut self.sidebar,
            &mut self.tool_form,
        ];
        if event_pump(ev, &mut components)?.is_consumed() {
            return Ok(());
        }

        // Outside-popup click guard. After the inner pump returns `NotConsumed`,
        // a click that didn't land inside any visible popup must still dismiss the
        // topmost one. Without this, a click outside every popup falls through to
        // the underlying pane — wrong when the user just wanted to dismiss the popup.
        // Wheel events deliberately skip this path: a wheel that misses every scrollable
        // target is just a stray scroll, not a dismissal.
        if let Event::Mouse(m) = ev
            && matches!(m.kind, MouseEventKind::Down(MouseButton::Left))
        {
            self.close_top_most_visible_popup();
        }

        if let Event::Key(k) = ev {
            let keys = &self.key_config.keys;
            if key_match(k, keys.palette) {
                self.queue.push(InternalEvent::OpenPalette);
            } else if key_match(k, keys.theme) {
                self.queue.push(InternalEvent::ShowThemePicker);
            } else if key_match(k, keys.help) {
                self.queue.push(InternalEvent::ShowHelp);
            } else if key_match(k, keys.copy) && self.focus == Focus::Workspace {
                if let Some(text) = self.tool_form.focused_value() {
                    self.queue.push(InternalEvent::CopyToClipboard(text));
                }
            } else if key_match(k, keys.open_file) {
                if self.tool_form.accepts_file_input() {
                    self.file_open.show()?;
                } else {
                    self.msg_popup
                        .show_error("this tool has no input to open a file into".to_string());
                }
            } else if key_match(k, keys.save_file) {
                if let Some(text) = self.tool_form.focused_value() {
                    self.queue.push(InternalEvent::SaveOutput(text));
                }
            } else if key_match(k, keys.quit) {
                self.queue.push(InternalEvent::Quit);
            } else if key_match(k, keys.focus_next) || key_match(k, keys.focus_prev) {
                self.toggle_focus();
            } else if key_match(k, keys.focus_sidebar) {
                self.set_focus(Focus::Sidebar);
            }
        }
        Ok(())
    }

    fn toggle_focus(&mut self) {
        self.set_focus(match self.focus {
            Focus::Sidebar => Focus::Workspace,
            Focus::Workspace => Focus::Sidebar,
        });
    }

    fn set_focus(&mut self, focus: Focus) {
        self.focus = focus;
        self.sidebar.set_focused(self.focus == Focus::Sidebar);
        self.tool_form.set_focused(self.focus == Focus::Workspace);
    }

    /// Hides the topmost visible popup — `msg_popup` is checked first because it
    /// draws last (top of the stack), then `theme_popup`, `file_save`, `file_open`,
    /// `help_popup`, `palette`. No-op when no popup is visible.
    ///
    /// Used by the outside-click guard in `event`.
    /// The msg popup is checked first because it draws at the very top of the stack —
    /// a config error has to win over every other popup so the user can always read it.
    fn close_top_most_visible_popup(&mut self) {
        if self.msg_popup.is_visible() {
            self.msg_popup.hide();
        } else if self.theme_popup.is_visible() {
            self.theme_popup.hide();
        } else if self.file_save.is_visible() {
            self.file_save.hide();
        } else if self.file_open.is_visible() {
            self.file_open.hide();
        } else if self.help_popup.is_visible() {
            self.help_popup.hide();
        } else if self.palette.is_visible() {
            self.palette.hide();
        }
    }

    /// Drains the queue. Returns flags indicating what needs updating.
    pub fn process_queue(&mut self) -> Result<NeedsUpdate> {
        let mut flags = NeedsUpdate::empty();
        while let Some(ev) = self.queue.pop() {
            self.needs_redraw = true;
            match ev {
                InternalEvent::SelectTool(id) => {
                    if let Some(tool) = self.registry.get(id) {
                        self.tool_form.set_tool(tool.spec());
                        // Also for the tool the *palette* just picked: without
                        // this the sidebar keeps highlighting the previous one,
                        // so the list and the open form disagree.
                        self.sidebar.select_tool(id);
                    }
                    flags |= NeedsUpdate::OUTPUT | NeedsUpdate::COMMANDS;
                }
                InternalEvent::InputChanged => {
                    self.tool_form.mark_dirty();
                    flags |= NeedsUpdate::OUTPUT;
                }
                InternalEvent::RunRequested => {
                    self.tool_form.request_run_now();
                    flags |= NeedsUpdate::OUTPUT;
                }
                InternalEvent::ShowMsg(m) => {
                    self.msg_popup.show_msg(m);
                    flags |= NeedsUpdate::ALL;
                }
                InternalEvent::OpenPalette => {
                    self.palette.show()?;
                    flags |= NeedsUpdate::ALL;
                }
                InternalEvent::ClosePalette => {
                    self.palette.hide();
                    flags |= NeedsUpdate::ALL;
                }
                InternalEvent::ShowThemePicker => {
                    self.theme_popup.show_with(&self.theme_id)?;
                    flags |= NeedsUpdate::ALL;
                }
                InternalEvent::PreviewTheme(id) => {
                    // Not stored in `theme_id`: a preview is not a choice, and
                    // cancelling has to leave nothing behind.
                    self.theme.set(self.settings.theme_for(id));
                    flags |= NeedsUpdate::ALL;
                }
                InternalEvent::ApplyTheme(id) => {
                    self.theme.set(self.settings.theme_for(id));
                    self.theme_id = id.to_string();
                    // Written on the way out, like the session — not here. The
                    // state worth keeping is the state you left.
                    self.theme_picked = true;
                    self.flash = Some(format!("theme: {id}"));
                    flags |= NeedsUpdate::ALL;
                }
                InternalEvent::ShowHelp => {
                    // Content generated from `commands()` right when opening, not a hardcoded list.
                    let cmds = self.all_commands();
                    self.help_popup.set_cmds(cmds);
                    self.help_popup.show()?;
                    flags |= NeedsUpdate::ALL;
                }
                InternalEvent::CopyToClipboard(text) => {
                    // Failure must state the reason clearly — no panic, no silent failure.
                    // Success names the backend: over SSH the text lands in the
                    // *terminal's* clipboard, and that is worth saying out loud.
                    match clipboard::copy(&text) {
                        Ok(backend) => self.flash = Some(backend.flash().to_string()),
                        Err(e) => self.msg_popup.show_error(e),
                    }
                    flags |= NeedsUpdate::ALL;
                }
                InternalEvent::OpenFile(path) => {
                    self.open_file(&path);
                    flags |= NeedsUpdate::ALL;
                }
                InternalEvent::SaveOutput(text) => {
                    self.file_save.open_with(text);
                    flags |= NeedsUpdate::ALL;
                }
                InternalEvent::Quit => self.should_quit = true,
                InternalEvent::FocusPane(focus) => {
                    self.set_focus(focus);
                    flags |= NeedsUpdate::COMMANDS;
                }
            }
        }
        Ok(flags)
    }

    /// What the next run should reopen with. Empty when the open tool has been
    /// closed, or when persistence is off.
    pub fn session_snapshot(&self) -> Session {
        let Some(spec) = self
            .tool_form
            .tool_id()
            .and_then(|id| self.registry.get(id))
            .map(Tool::spec)
            .filter(|_| !self.settings.restore.is_off())
        else {
            return Session::default();
        };
        Session {
            tool: Some(spec.id.to_string()),
            values: session::capture(spec, &self.tool_form.inputs(), self.settings.restore),
        }
    }

    /// The theme pick as it would be written down, or `None` when this run has
    /// nothing to say about the theme — nobody opened the picker, or they
    /// cancelled out of it. Public for the same reason `session_snapshot` is:
    /// a test can drive the choice through the real key handling and then
    /// write it wherever it likes.
    pub fn theme_snapshot(&self) -> Option<ThemeState> {
        self.theme_picked
            .then(|| ThemeState::new(self.theme_id.clone(), self.settings.theme_name.clone()))
    }

    /// Writes the theme pick out, on the way out of the TUI.
    pub fn persist_theme(&self) -> std::io::Result<()> {
        match self.theme_snapshot() {
            Some(state) => state.save(),
            None => Ok(()),
        }
    }

    /// Writes the session out. Called on the way out of the TUI.
    ///
    /// With persistence off this **removes** any file an earlier setting left
    /// behind — switching it off has to mean the data is gone, not just unread.
    pub fn persist_session(&self) -> std::io::Result<()> {
        if self.settings.restore.is_off() {
            return Session::clear();
        }
        self.session_snapshot().save()
    }

    /// Loads a file's content into the currently open tool's primary input.
    ///
    /// Reading the file is the **UI layer's** job — the tool still only receives/returns
    /// plain text, which is why `run()` remains testable without a filesystem.
    pub fn open_file(&mut self, path: &std::path::Path) {
        if let Err(msg) = crate::popups::file_open::check_openable(path) {
            self.msg_popup.show_error(msg);
            return;
        }
        match std::fs::read_to_string(path) {
            Ok(text) => self.tool_form.set_primary_input(&text),
            Err(e) => self
                .msg_popup
                .show_error(format!("couldn't read {}: {e}", path.display())),
        }
        self.needs_redraw = true;
    }

    /// Runs the tool once the debounce deadline is reached. Called every loop iteration —
    /// because `event::poll` has a timeout, the output updates automatically after typing
    /// stops, with no extra keypress needed.
    pub fn tick(&mut self) {
        if !self.tool_form.take_run_request() {
            return;
        }
        let Some(id) = self.tool_form.tool_id() else {
            return;
        };
        let inputs = self.tool_form.inputs();
        let result = self.registry.run(id, &inputs);
        self.tool_form.set_result(result);
        self.needs_redraw = true;
    }
}
