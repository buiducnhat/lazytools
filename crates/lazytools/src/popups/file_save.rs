//! Lưu giá trị output đang focus ra file.
//!
//! Ghi đè file của người dùng là thao tác **không thể hoàn tác** — chỗ duy nhất
//! có tính chất đó trong toàn plan — nên bước xác nhận là bắt buộc, không phải
//! tùy chọn. Thư mục cha không tồn tại thì **báo lỗi**, không tự tạo.

use std::path::{Path, PathBuf};

use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

use crate::components::field::textarea::TextArea;
use crate::components::{CommandBlocking, CommandInfo, Component, DrawableComponent, EventState};
use crate::keys::{KeyConfig, key_match, typed_char};
use crate::queue::{InternalEvent, Queue};
use crate::ui::{SharedTheme, centered_rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    EnteringPath,
    /// File đã tồn tại — phải trả lời trước khi ghi.
    ConfirmOverwrite,
}

pub struct FileSavePopup {
    path_input: TextArea,
    content: String,
    stage: Stage,
    visible: bool,
    error: Option<String>,
    queue: Queue,
    theme: SharedTheme,
    key_config: KeyConfig,
}

impl FileSavePopup {
    pub fn new(queue: Queue, theme: SharedTheme, key_config: KeyConfig) -> Self {
        Self {
            path_input: TextArea::new(true),
            content: String::new(),
            stage: Stage::EnteringPath,
            visible: false,
            error: None,
            queue,
            theme,
            key_config,
        }
    }

    /// Mở popup cho một nội dung cụ thể.
    pub fn open_with(&mut self, content: String) {
        self.content = content;
        self.path_input.set_value("");
        self.stage = Stage::EnteringPath;
        self.error = None;
        self.visible = true;
    }

    fn target(&self) -> PathBuf {
        PathBuf::from(self.path_input.value().trim())
    }

    /// Kiểm tra rồi hoặc ghi luôn, hoặc chuyển sang bước xác nhận ghi đè.
    fn submit(&mut self) {
        let path = self.target();
        if path.as_os_str().is_empty() {
            self.error = Some("hãy nhập đường dẫn".to_string());
            return;
        }

        // Thư mục cha phải có sẵn — không tự tạo thay người dùng.
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty())
            && !parent.is_dir()
        {
            self.error = Some(format!("thư mục {} không tồn tại", parent.display()));
            return;
        }

        if path.exists() {
            self.stage = Stage::ConfirmOverwrite;
            self.error = None;
            return;
        }
        self.write(&path);
    }

    fn write(&mut self, path: &Path) {
        match std::fs::write(path, &self.content) {
            Ok(()) => {
                self.queue
                    .push(InternalEvent::ShowMsg(format!("đã lưu {}", path.display())));
                self.hide();
            }
            Err(e) => {
                self.error = Some(format!("không ghi được: {e}"));
                self.stage = Stage::EnteringPath;
            }
        }
    }
}

impl DrawableComponent for FileSavePopup {
    fn draw(&self, f: &mut Frame, rect: Rect) -> Result<()> {
        if !self.visible {
            return Ok(());
        }
        let area = centered_rect(60, 30, rect);
        f.render_widget(Clear, area);

        let block = Block::bordered()
            .border_style(self.theme.block(true))
            .title_style(self.theme.title(true))
            .title(" Lưu ra file ");
        let inner = block.inner(area);
        f.render_widget(block, area);

        // Vùng thông báo phải nhiều dòng: đường dẫn dài làm lỗi bị cắt mất chữ.
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(inner);

        let path = self.path_input.value();
        f.render_widget(
            Paragraph::new(format!("> {path}")).style(self.theme.text()),
            rows[0],
        );

        match self.stage {
            Stage::EnteringPath => {
                f.set_cursor_position((rows[0].x + 2 + path.chars().count() as u16, rows[0].y));
            }
            Stage::ConfirmOverwrite => {
                f.render_widget(
                    Paragraph::new(format!(
                        "file đã tồn tại — ghi đè? {} để xác nhận, {} để hủy",
                        self.key_config.hint(self.key_config.keys.confirm),
                        self.key_config.hint(self.key_config.keys.exit_popup)
                    ))
                    .style(self.theme.error())
                    .wrap(Wrap { trim: false }),
                    rows[1],
                );
            }
        }

        if let Some(err) = &self.error {
            f.render_widget(
                Paragraph::new(err.as_str())
                    .style(self.theme.error())
                    .wrap(Wrap { trim: false }),
                rows[1],
            );
        }
        Ok(())
    }
}

impl Component for FileSavePopup {
    fn commands(&self, out: &mut Vec<CommandInfo>, _force_all: bool) -> CommandBlocking {
        if self.visible {
            let keys = &self.key_config.keys;
            let label = if self.stage == Stage::ConfirmOverwrite {
                "ghi đè"
            } else {
                "lưu"
            };
            out.push(CommandInfo::new(self.key_config.hint(keys.confirm), label, "File").order(1));
            out.push(
                CommandInfo::new(self.key_config.hint(keys.exit_popup), "hủy", "File").order(3),
            );
            return CommandBlocking::Blocking;
        }
        CommandBlocking::PassingOn
    }

    fn event(&mut self, ev: &Event) -> Result<EventState> {
        if !self.visible {
            return Ok(EventState::NotConsumed);
        }

        if let Event::Paste(text) = ev
            && self.stage == Stage::EnteringPath
        {
            self.path_input.insert_str(text);
            return Ok(EventState::Consumed);
        }

        let Event::Key(k) = ev else {
            return Ok(EventState::Consumed);
        };
        let b = &self.key_config.keys;

        match self.stage {
            Stage::ConfirmOverwrite => {
                if key_match(k, b.confirm) {
                    let path = self.target();
                    self.write(&path);
                } else if key_match(k, b.exit_popup) {
                    // Hủy đưa về bước nhập, không đóng hẳn — người dùng còn sửa được.
                    self.stage = Stage::EnteringPath;
                }
            }
            Stage::EnteringPath => {
                if key_match(k, b.exit_popup) {
                    self.hide();
                } else if key_match(k, b.confirm) {
                    self.submit();
                } else if key_match(k, b.backspace) {
                    self.path_input.backspace();
                } else if key_match(k, b.delete_to_start) {
                    self.path_input.delete_to_line_start();
                } else if key_match(k, b.move_left) {
                    self.path_input.move_left();
                } else if key_match(k, b.move_right) {
                    self.path_input.move_right();
                } else if let Some(c) = typed_char(k) {
                    self.path_input.insert_char(c);
                }
            }
        }
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
        self.stage = Stage::EnteringPath;
    }
}
