use std::rc::Rc;

use anyhow::Result;
use lazytools_core::registry::Registry;
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Paragraph};

use crate::components::cmdbar::CommandBar;
use crate::components::sidebar::Sidebar;
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
    cmdbar: CommandBar,
    msg_popup: MsgPopup,
    focus: Focus,
    should_quit: bool,
    needs_redraw: bool,
    /// Tool đang mở. Phase 2B thay bằng `ToolFormComponent` thật.
    current_tool: Option<&'static str>,
}

impl App {
    pub fn new(registry: Registry) -> Self {
        let queue = Queue::default();
        let theme: SharedTheme = Rc::new(Theme::default());
        let key_config = KeyConfig::default();

        let sidebar = Sidebar::new(&registry, queue.clone(), theme.clone(), key_config);
        let current_tool = sidebar.selected_tool();

        Self {
            registry,
            queue,
            key_config,
            sidebar,
            cmdbar: CommandBar::new(theme.clone()),
            msg_popup: MsgPopup::new(theme.clone(), key_config),
            theme,
            focus: Focus::Sidebar,
            should_quit: false,
            needs_redraw: true,
            current_tool,
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
        self.draw_workspace(f, cols[1]);

        self.refresh_commands();
        self.cmdbar.draw(f, cmdbar_area)?;

        // Popup vẽ sau cùng để nằm trên mọi thứ.
        self.msg_popup.draw(f, area)?;
        Ok(())
    }

    /// Phase 2B thay bằng `ToolFormComponent`. 2A chỉ chứng minh pane tồn tại
    /// và biết tool nào đang được chọn.
    fn draw_workspace(&self, f: &mut Frame, rect: Rect) {
        let focused = self.focus == Focus::Workspace;
        let name = self
            .current_tool
            .and_then(|id| self.registry.get(id))
            .map(|t| t.spec().name)
            .unwrap_or("—");

        let block = Block::bordered()
            .border_style(self.theme.block(focused))
            .title_style(self.theme.title(focused))
            .title(format!(" {name} "));

        f.render_widget(
            Paragraph::new("Form của tool xuất hiện ở Phase 2B.")
                .style(self.theme.dim())
                .block(block),
            rect,
        );
    }

    fn refresh_commands(&mut self) {
        let mut cmds = Vec::new();
        let components: Vec<&dyn Component> = vec![&self.msg_popup, &self.sidebar];
        command_pump(&mut cmds, false, &components);
        cmds.extend(self.app_commands());
        self.cmdbar.set_cmds(cmds);
    }

    /// Lệnh cấp app — luôn hiện, luôn lấy chuỗi phím từ `KeyConfig`.
    fn app_commands(&self) -> Vec<CommandInfo> {
        let keys = &self.key_config.keys;
        vec![
            CommandInfo::new(self.key_config.hint(keys.focus_next), "đổi vùng", "App").order(50),
            CommandInfo::new(self.key_config.hint(keys.quit), "thoát", "App").order(99),
        ]
    }

    pub fn event(&mut self, ev: &Event) -> Result<()> {
        self.needs_redraw = true;

        // Thứ tự định tuyến: popups → pane đang focus. Palette xen vào giữa ở P3.
        let mut components: Vec<&mut dyn Component> = vec![&mut self.msg_popup, &mut self.sidebar];
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
    }

    /// Rút cạn queue. Trả về cờ cho biết cần cập nhật những gì.
    pub fn process_queue(&mut self) -> Result<NeedsUpdate> {
        let mut flags = NeedsUpdate::empty();
        while let Some(ev) = self.queue.pop() {
            self.needs_redraw = true;
            match ev {
                InternalEvent::SelectTool(id) => {
                    self.current_tool = Some(id);
                    flags |= NeedsUpdate::OUTPUT | NeedsUpdate::COMMANDS;
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
}
