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
    palette: Palette,
    cmdbar: CommandBar,
    msg_popup: MsgPopup,
    help_popup: HelpPopup,
    file_open: FileOpenPopup,
    file_save: FileSavePopup,
    focus: Focus,
    should_quit: bool,
    needs_redraw: bool,
    /// Thông báo thoáng qua ở cmdbar (ví dụ "đã copy").
    flash: Option<String>,
}

impl App {
    /// Dùng phím mặc định — cho test và cho trường hợp không có HOME.
    pub fn new(registry: Registry) -> Self {
        Self::with_key_config(registry, KeyConfig::default(), None)
    }

    /// Đọc `~/.config/lazytools/keys.toml`. Config hỏng **không** chặn khởi
    /// động: app vẫn mở, kèm popup nói rõ vấn đề để người dùng vào sửa được.
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

        // Mở sẵn tool đầu tiên để màn hình đầu không trống.
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

        // Overlay vẽ sau cùng để nằm trên mọi thứ.
        self.palette.draw(f, area)?;
        self.help_popup.draw(f, area)?;
        self.file_open.draw(f, area)?;
        self.file_save.draw(f, area)?;
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

    /// Toàn bộ lệnh, kể cả của component đang ẩn — nội dung cho help popup.
    fn all_commands(&self) -> Vec<CommandInfo> {
        let mut cmds = Vec::new();
        command_pump(&mut cmds, true, &self.components());
        cmds.extend(self.app_commands());
        cmds
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
        cmds.push(CommandInfo::new(self.key_config.hint(keys.palette), "palette", "App").order(55));
        if self.focus == Focus::Workspace {
            cmds.push(CommandInfo::new(self.key_config.hint(keys.copy), "copy", "App").order(58));
        }
        cmds.push(
            CommandInfo::new(self.key_config.hint(keys.open_file), "mở file", "App").order(56),
        );
        if self.focus == Focus::Workspace {
            cmds.push(
                CommandInfo::new(self.key_config.hint(keys.save_file), "lưu file", "App").order(57),
            );
        }
        cmds.push(CommandInfo::new(self.key_config.hint(keys.help), "trợ giúp", "App").order(60));
        cmds.push(CommandInfo::new(self.key_config.hint(keys.quit), "thoát", "App").order(99));
        cmds
    }

    pub fn event(&mut self, ev: &Event) -> Result<()> {
        self.needs_redraw = true;
        // Thông báo thoáng qua biến mất ở thao tác kế tiếp.
        self.flash = None;

        // Thứ tự định tuyến: popups → palette → pane đang focus.
        // Mọi component tự trả `NotConsumed` khi không được focus/hiển thị.
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
                self.file_open.show()?;
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
                InternalEvent::OpenPalette => {
                    self.palette.show()?;
                    flags |= NeedsUpdate::ALL;
                }
                InternalEvent::ClosePalette => {
                    self.palette.hide();
                    flags |= NeedsUpdate::ALL;
                }
                InternalEvent::ShowHelp => {
                    // Nội dung sinh từ `commands()` ngay lúc mở, không phải danh sách cứng.
                    let cmds = self.all_commands();
                    self.help_popup.set_cmds(cmds);
                    self.help_popup.show()?;
                    flags |= NeedsUpdate::ALL;
                }
                InternalEvent::CopyToClipboard(text) => {
                    // Thất bại phải nói rõ lý do, không panic và không im lặng.
                    match clipboard::copy(&text) {
                        Ok(()) => self.flash = Some("đã copy".to_string()),
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

    /// Nạp nội dung file vào input chính của tool đang mở.
    ///
    /// Đọc file là việc của **tầng UI** — tool vẫn chỉ nhận/trả text thuần, nhờ
    /// vậy `run()` còn test được mà không cần filesystem.
    pub fn open_file(&mut self, path: &std::path::Path) {
        if let Err(msg) = crate::popups::file_open::check_openable(path) {
            self.msg_popup.show_error(msg);
            return;
        }
        match std::fs::read_to_string(path) {
            Ok(text) => self.tool_form.set_primary_input(&text),
            Err(e) => self
                .msg_popup
                .show_error(format!("không đọc được {}: {e}", path.display())),
        }
        self.needs_redraw = true;
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
