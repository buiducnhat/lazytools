//! Hai trait cốt lõi, mượn hình dạng từ gitui. Mọi thứ vẽ được là
//! `DrawableComponent`; mọi thứ nhận phím và tự khai báo lệnh là `Component`.

pub mod cmdbar;
pub mod sidebar;

use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::Rect;

pub trait DrawableComponent {
    fn draw(&self, f: &mut Frame, rect: Rect) -> Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventState {
    Consumed,
    NotConsumed,
}

impl EventState {
    pub fn is_consumed(self) -> bool {
        self == Self::Consumed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandBlocking {
    /// Component đang chiếm phím — không hỏi các component sau nữa.
    Blocking,
    PassingOn,
}

/// Một lệnh mà component công bố. cmdbar và help popup đều dựng từ đây, nên
/// hint hiển thị không bao giờ lệch với code xử lý phím thật.
#[derive(Debug, Clone)]
pub struct CommandInfo {
    /// Chuỗi phím đã render sẵn, ví dụ `^P` — luôn lấy từ `KeyConfig`.
    pub key: String,
    pub label: &'static str,
    /// Nhóm trong help popup — help popup được dựng ở Phase 03.
    pub group: &'static str,
    pub enabled: bool,
    pub order: i8,
}

impl CommandInfo {
    pub fn new(key: String, label: &'static str, group: &'static str) -> Self {
        Self {
            key,
            label,
            group,
            enabled: true,
            order: 0,
        }
    }

    pub fn order(mut self, order: i8) -> Self {
        self.order = order;
        self
    }
}

pub trait Component {
    fn commands(&self, out: &mut Vec<CommandInfo>, force_all: bool) -> CommandBlocking;
    fn event(&mut self, ev: &Event) -> Result<EventState>;
    fn focused(&self) -> bool;
    fn set_focused(&mut self, focused: bool);

    fn is_visible(&self) -> bool {
        true
    }
    fn hide(&mut self) {}
    /// Dùng bởi các popup mở theo yêu cầu (palette/help ở P3, file picker ở P5).
    fn show(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Đưa event lần lượt qua các component, dừng ở component đầu tiên tiêu thụ nó.
pub fn event_pump(ev: &Event, components: &mut [&mut dyn Component]) -> Result<EventState> {
    for c in components.iter_mut() {
        if c.event(ev)?.is_consumed() {
            return Ok(EventState::Consumed);
        }
    }
    Ok(EventState::NotConsumed)
}

/// Gom `CommandInfo` của các component đang hiển thị, dừng ở component `Blocking`.
pub fn command_pump(out: &mut Vec<CommandInfo>, force_all: bool, components: &[&dyn Component]) {
    for c in components {
        if !c.is_visible() && !force_all {
            continue;
        }
        if c.commands(out, force_all) == CommandBlocking::Blocking {
            break;
        }
    }
}
