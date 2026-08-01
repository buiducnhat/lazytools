use std::rc::Rc;

use anyhow::Result;
use lazytools_core::registry::Registry;
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::Block;

use crate::clipboard;
use crate::components::cmdbar::CommandBar;
use crate::components::palette::Palette;
use crate::components::sidebar::Sidebar;
use crate::components::tool_form::ToolFormComponent;
use crate::components::{CommandInfo, Component, DrawableComponent, command_pump, event_pump};
use crate::keys::{KeyConfig, key_match};
use crate::popups::{FileOpenPopup, FileSavePopup, HelpPopup, MsgPopup};
use crate::queue::{InternalEvent, NeedsUpdate, Queue};
use crate::ui::{SharedTheme, Theme};

/// Responsive breakpoints: below 80 cols the sidebar shrinks to icons; below 60 it's hidden entirely.
const SIDEBAR_WIDTH: u16 = 24;
const SIDEBAR_WIDTH_NARROW: u16 = 6;
const BREAKPOINT_NARROW: u16 = 80;
const BREAKPOINT_HIDE: u16 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Sidebar,
    Workspace,
}

pub struct App {
    registry: Registry,
    queue: Queue,
    theme: SharedTheme,
    key_config: KeyConfig,
    sidebar: Sidebar,
    tool_form: ToolFormComponent,
    palette: Palette,
    cmdbar: CommandBar,
    msg_popup: MsgPopup,
    help_popup: HelpPopup,
    file_open: FileOpenPopup,
    file_save: FileSavePopup,
    focus: Focus,
    should_quit: bool,
    needs_redraw: bool,
    /// Transient message in the cmdbar (e.g. "copied").
    flash: Option<String>,
}

impl App {
    /// Uses default keys — for tests and for cases where there's no HOME.
    pub fn new(registry: Registry) -> Self {
        Self::with_key_config(registry, KeyConfig::default(), None)
    }

    /// Reads `~/.config/lazytools/keys.toml`. A broken config does **not** block
    /// startup: the app still opens, with a popup explaining the issue so the user can go fix it.
    pub fn from_user_config(registry: Registry) -> Self {
        let (key_config, issue) = KeyConfig::load();
        Self::with_key_config(registry, key_config, issue.map(|i| i.message()))
    }

    fn with_key_config(
        registry: Registry,
        key_config: KeyConfig,
        config_issue: Option<String>,
    ) -> Self {
        let queue = Queue::default();
        let theme: SharedTheme = Rc::new(Theme::default());

        let sidebar = Sidebar::new(&registry, queue.clone(), theme.clone(), key_config);
        let mut tool_form = ToolFormComponent::new(queue.clone(), theme.clone(), key_config);

        // Open the first tool by default so the initial screen isn't empty.
        if let Some(tool) = sidebar.selected_tool().and_then(|id| registry.get(id)) {
            tool_form.set_tool(tool.spec());
        }

        let palette = Palette::new(&registry, queue.clone(), theme.clone(), key_config);
        let mut msg_popup = MsgPopup::new(theme.clone(), key_config);
        if let Some(msg) = config_issue {
            msg_popup.show_error(msg);
        }

        let file_open = FileOpenPopup::new(queue.clone(), theme.clone(), key_config);
        let file_save = FileSavePopup::new(queue.clone(), theme.clone(), key_config);

        Self {
            registry,
            queue,
            key_config,
            sidebar,
            tool_form,
            palette,
            cmdbar: CommandBar::new(theme.clone()),
            msg_popup,
            help_popup: HelpPopup::new(theme.clone(), key_config),
            file_open,
            file_save,
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
            &self.palette,
            &self.sidebar,
            &self.tool_form,
        ]
    }

    fn refresh_commands(&mut self) {
        let mut cmds = Vec::new();
        command_pump(&mut cmds, false, &self.components());
        cmds.extend(self.app_commands());
        self.cmdbar.set_cmds(cmds);
    }

    /// All commands, including those from hidden components — content for the help popup.
    fn all_commands(&self) -> Vec<CommandInfo> {
        let mut cmds = Vec::new();
        command_pump(&mut cmds, true, &self.components());
        cmds.extend(self.app_commands());
        cmds
    }

    /// App-level commands — always takes the key string from `KeyConfig`.
    fn app_commands(&self) -> Vec<CommandInfo> {
        let keys = &self.key_config.keys;
        let mut cmds = Vec::new();
        // In the form, `Tab` is already advertised as "next field" — don't show a duplicate key.
        if self.focus == Focus::Sidebar {
            cmds.push(
                CommandInfo::new(self.key_config.hint(keys.focus_next), "switch pane", "App")
                    .order(50),
            );
        }
        cmds.push(CommandInfo::new(self.key_config.hint(keys.palette), "palette", "App").order(55));
        if self.focus == Focus::Workspace {
            cmds.push(CommandInfo::new(self.key_config.hint(keys.copy), "copy", "App").order(58));
        }
        // Only advertised when it would actually do something — a hint for a key that
        // does nothing is worse than no hint at all.
        if self.tool_form.accepts_file_input() {
            cmds.push(
                CommandInfo::new(self.key_config.hint(keys.open_file), "open file", "App")
                    .order(56),
            );
        }
        if self.focus == Focus::Workspace {
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
            &mut self.palette,
            &mut self.sidebar,
            &mut self.tool_form,
        ];
        if event_pump(ev, &mut components)?.is_consumed() {
            return Ok(());
        }

        if let Event::Key(k) = ev {
            let keys = &self.key_config.keys;
            if key_match(k, keys.palette) {
                self.queue.push(InternalEvent::OpenPalette);
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
            }
        }
        Ok(())
    }

    fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Sidebar => Focus::Workspace,
            Focus::Workspace => Focus::Sidebar,
        };
        self.sidebar.set_focused(self.focus == Focus::Sidebar);
        self.tool_form.set_focused(self.focus == Focus::Workspace);
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
                InternalEvent::ShowError(e) => {
                    self.msg_popup.show_error(e.to_string());
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
                InternalEvent::ShowHelp => {
                    // Content generated from `commands()` right when opening, not a hardcoded list.
                    let cmds = self.all_commands();
                    self.help_popup.set_cmds(cmds);
                    self.help_popup.show()?;
                    flags |= NeedsUpdate::ALL;
                }
                InternalEvent::CopyToClipboard(text) => {
                    // Failure must state the reason clearly — no panic, no silent failure.
                    match clipboard::copy(&text) {
                        Ok(()) => self.flash = Some("copied".to_string()),
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
            }
        }
        Ok(flags)
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
