//! Trình duyệt thư mục tối giản để nạp file vào input chính của tool.
//!
//! Mọi trường hợp hệ thống file khó chịu đều phải xử lý chứ **không panic**:
//! không quyền đọc, symlink hỏng, tên file không phải UTF-8, thư mục rỗng.

use std::path::{Path, PathBuf};

use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph};

use crate::components::{CommandBlocking, CommandInfo, Component, DrawableComponent, EventState};
use crate::keys::{KeyConfig, key_match};
use crate::queue::{InternalEvent, Queue};
use crate::ui::{SharedTheme, centered_rect};

/// Nạp file lớn vào `TextArea` sẽ làm TUI không dùng nổi. Ngưỡng 256KB của P2
/// chỉ hạ xuống `OnDemand` chứ không giải quyết chi phí render, nên chặn ở đây.
pub const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;

/// File có nạp vào được không. Trả về lý do đọc được nếu không.
///
/// Symlink hỏng và thiếu quyền đọc đều rơi vào nhánh `Err` của `metadata` —
/// xử lý thành thông báo chứ không panic.
pub fn check_openable(path: &Path) -> Result<(), String> {
    match std::fs::metadata(path) {
        Ok(meta) if meta.len() > MAX_FILE_BYTES => Err(format!(
            "file {:.1}MB vượt giới hạn {}MB",
            meta.len() as f64 / 1024.0 / 1024.0,
            MAX_FILE_BYTES / 1024 / 1024
        )),
        Ok(_) => Ok(()),
        Err(e) => Err(format!("không đọc được file: {e}")),
    }
}

struct FsEntry {
    label: String,
    path: PathBuf,
    is_dir: bool,
}

pub struct FileOpenPopup {
    cwd: PathBuf,
    entries: Vec<FsEntry>,
    selected: usize,
    visible: bool,
    error: Option<String>,
    queue: Queue,
    theme: SharedTheme,
    key_config: KeyConfig,
}

impl FileOpenPopup {
    pub fn new(queue: Queue, theme: SharedTheme, key_config: KeyConfig) -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut popup = Self {
            cwd,
            entries: Vec::new(),
            selected: 0,
            visible: false,
            error: None,
            queue,
            theme,
            key_config,
        };
        popup.reload();
        popup
    }

    /// Đọc lại thư mục hiện tại. Lỗi trở thành thông báo trong popup, không panic.
    fn reload(&mut self) {
        self.selected = 0;
        self.entries.clear();

        if self.cwd.parent().is_some() {
            self.entries.push(FsEntry {
                label: "../".to_string(),
                path: self.cwd.join(".."),
                is_dir: true,
            });
        }

        let read = match std::fs::read_dir(&self.cwd) {
            Ok(r) => r,
            Err(e) => {
                self.error = Some(format!("không đọc được thư mục: {e}"));
                return;
            }
        };

        let mut items: Vec<FsEntry> = Vec::new();
        for entry in read.flatten() {
            let path = entry.path();
            // `is_dir()` theo symlink và trả `false` với symlink hỏng — đúng thứ
            // ta muốn: không sập, chỉ coi như file thường rồi báo lỗi lúc mở.
            let is_dir = path.is_dir();
            // Tên không phải UTF-8 vẫn hiện được nhờ `to_string_lossy`.
            let name = entry.file_name().to_string_lossy().into_owned();
            items.push(FsEntry {
                label: if is_dir { format!("{name}/") } else { name },
                path,
                is_dir,
            });
        }

        // Thư mục trước, rồi tới file; trong mỗi nhóm sắp theo tên.
        items.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.label.cmp(&b.label)));
        self.entries.extend(items);
        self.error = None;
    }

    fn move_selection(&mut self, delta: isize) {
        if self.entries.is_empty() {
            return;
        }
        let len = self.entries.len() as isize;
        self.selected = ((self.selected as isize + delta).rem_euclid(len)) as usize;
    }

    fn go_up(&mut self) {
        if let Some(parent) = self.cwd.parent() {
            self.cwd = parent.to_path_buf();
            self.reload();
        }
    }

    fn activate(&mut self) {
        let Some(entry) = self.entries.get(self.selected) else {
            return;
        };
        let (path, is_dir) = (entry.path.clone(), entry.is_dir);

        if is_dir {
            // `canonicalize` gọn hóa `..`; thất bại thì dùng nguyên đường dẫn.
            self.cwd = std::fs::canonicalize(&path).unwrap_or(path);
            self.reload();
            return;
        }

        match check_openable(&path) {
            Ok(()) => {
                self.queue.push(InternalEvent::OpenFile(path));
                self.hide();
            }
            Err(msg) => self.error = Some(msg),
        }
    }

    pub fn current_dir(&self) -> &Path {
        &self.cwd
    }
}

impl DrawableComponent for FileOpenPopup {
    fn draw(&self, f: &mut Frame, rect: Rect) -> Result<()> {
        if !self.visible {
            return Ok(());
        }
        let area = centered_rect(70, 70, rect);
        f.render_widget(Clear, area);

        let block = Block::bordered()
            .border_style(self.theme.block(true))
            .title_style(self.theme.title(true))
            .title(" Mở file ");
        let inner = block.inner(area);
        f.render_widget(block, area);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(inner);

        f.render_widget(
            Paragraph::new(self.cwd.display().to_string()).style(self.theme.dim()),
            rows[0],
        );

        let items: Vec<ListItem> = if self.entries.is_empty() {
            vec![ListItem::new(Line::from(Span::styled(
                "(thư mục rỗng)",
                self.theme.dim(),
            )))]
        } else {
            self.entries
                .iter()
                .map(|e| {
                    let style = if e.is_dir {
                        self.theme.group()
                    } else {
                        self.theme.text()
                    };
                    ListItem::new(Line::from(Span::styled(e.label.clone(), style)))
                })
                .collect()
        };

        let mut state = ListState::default();
        if !self.entries.is_empty() {
            state.select(Some(self.selected));
        }
        f.render_stateful_widget(
            List::new(items).highlight_style(self.theme.selection()),
            rows[1],
            &mut state,
        );

        if let Some(err) = &self.error {
            f.render_widget(
                Paragraph::new(err.as_str()).style(self.theme.error()),
                rows[2],
            );
        }
        Ok(())
    }
}

impl Component for FileOpenPopup {
    fn commands(&self, out: &mut Vec<CommandInfo>, _force_all: bool) -> CommandBlocking {
        if self.visible {
            let keys = &self.key_config.keys;
            out.push(CommandInfo::new(self.key_config.hint(keys.confirm), "mở", "File").order(1));
            out.push(
                CommandInfo::new(self.key_config.hint(keys.backspace), "lên cấp", "File").order(2),
            );
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
        let Event::Key(k) = ev else {
            return Ok(EventState::Consumed);
        };
        let b = &self.key_config.keys;

        if key_match(k, b.exit_popup) {
            self.hide();
        } else if key_match(k, b.confirm) {
            self.activate();
        } else if key_match(k, b.backspace) || key_match(k, b.move_left) {
            self.go_up();
        } else if key_match(k, b.move_down) || key_match(k, b.move_down_alt) {
            self.move_selection(1);
        } else if key_match(k, b.move_up) || key_match(k, b.move_up_alt) {
            self.move_selection(-1);
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
    }

    fn show(&mut self) -> Result<()> {
        self.visible = true;
        self.error = None;
        self.reload();
        Ok(())
    }
}
