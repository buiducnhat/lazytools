use std::rc::Rc;

use anyhow::Result;
use lazytools_core::registry::Registry;
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::Block;

use crate::components::cmdbar::CommandBar;
use crate::components::sidebar::Sidebar;
use crate::components::tool_form::ToolFormComponent;
use crate::components::{CommandInfo, Component, DrawableComponent, command_pump, event_pump};
use crate::keys::{KeyConfig, key_match};
use crate::popups::MsgPopup;
use crate::queue::{InternalEvent, NeedsUpdate, Queue};
use crate::ui::{SharedTheme, Theme};

/// Ngưỡng responsive: dưới 80 cols sidebar thu còn icon; dưới 60 ẩn hẳn.
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
    cmdbar: CommandBar,
    msg_popup: MsgPopup,
    focus: Focus,
    should_quit: bool,
    needs_redraw: bool,
}

impl App {
    pub fn new(registry: Registry) -> Self {
        let queue = Queue::default();
        let theme: SharedTheme = Rc::new(Theme::default());
        let key_config = KeyConfig::default();

        let sidebar = Sidebar::new(&registry, queue.clone(), theme.clone(), key_config);
        let mut tool_form = ToolFormComponent::new(queue.clone(), theme.clone(), key_config);

        // Mở sẵn tool đầu tiên để màn hình đầu không trống.
        if let Some(tool) = sidebar.selected_tool().and_then(|id| registry.get(id)) {
            tool_form.set_tool(tool.spec());
        }

        Self {
            registry,
            queue,
            key_config,
            sidebar,
            tool_form,
            cmdbar: CommandBar::new(theme.clone()),
            msg_popup: MsgPopup::new(theme.clone(), key_config),
            theme,
            focus: Focus::Sidebar,
            should_quit: false,
            needs_redraw: true,
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
        self.cmdbar.draw(f, cmdbar_area)?;

        // Popup vẽ sau cùng để nằm trên mọi thứ.
        self.msg_popup.draw(f, area)?;
        Ok(())
    }

    /// Khung của tool đang mở; ruột do `ToolFormComponent` tự dựng từ `ToolSpec`.
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

    fn refresh_commands(&mut self) {
        let mut cmds = Vec::new();
        let components: Vec<&dyn Component> = vec![&self.msg_popup, &self.sidebar, &self.tool_form];
        command_pump(&mut cmds, false, &components);
        cmds.extend(self.app_commands());
        self.cmdbar.set_cmds(cmds);
    }

    /// Lệnh cấp app — luôn lấy chuỗi phím từ `KeyConfig`.
    fn app_commands(&self) -> Vec<CommandInfo> {
        let keys = &self.key_config.keys;
        let mut cmds = Vec::new();
        // Trong form, `Tab` đã được công bố là "field kế" — không hiện trùng phím.
        if self.focus == Focus::Sidebar {
            cmds.push(
                CommandInfo::new(self.key_config.hint(keys.focus_next), "đổi vùng", "App")
                    .order(50),
            );
        }
        cmds.push(CommandInfo::new(self.key_config.hint(keys.quit), "thoát", "App").order(99));
        cmds
    }

    pub fn event(&mut self, ev: &Event) -> Result<()> {
        self.needs_redraw = true;

        // Thứ tự định tuyến: popups → pane đang focus. Palette xen vào giữa ở P3.
        // Sidebar và tool_form đều tự trả `NotConsumed` khi không được focus.
        let mut components: Vec<&mut dyn Component> =
            vec![&mut self.msg_popup, &mut self.sidebar, &mut self.tool_form];
        if event_pump(ev, &mut components)?.is_consumed() {
            return Ok(());
        }

        if let Event::Key(k) = ev {
            let keys = &self.key_config.keys;
            if key_match(k, keys.quit) {
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

    /// Rút cạn queue. Trả về cờ cho biết cần cập nhật những gì.
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
                InternalEvent::Quit => self.should_quit = true,
            }
        }
        Ok(flags)
    }

    /// Chạy tool khi tới hạn debounce. Gọi mỗi vòng lặp — vì `event::poll` có
    /// timeout nên output tự cập nhật sau khi ngừng gõ, không cần bấm thêm phím.
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
