//! Component generic đọc `ToolSpec` và dựng form. Không có tên tool nào ở đây —
//! đây là lý do thêm tool thứ 9 không tốn dòng UI nào.

use std::time::{Duration, Instant};

use anyhow::Result;
use lazytools_core::error::ToolError;
use lazytools_core::spec::{RunMode, ToolSpec};
use lazytools_core::value::{Inputs, Outputs, Value};
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph, Wrap};

use super::field::{FieldWidget, build};
use super::{CommandBlocking, CommandInfo, Component, DrawableComponent, EventState};
use crate::keys::{KeyConfig, key_match};
use crate::queue::{InternalEvent, Queue};
use crate::ui::SharedTheme;

/// Chờ ngừng gõ bao lâu rồi mới chạy lại tool.
const DEBOUNCE: Duration = Duration::from_millis(80);

/// Trên ngưỡng này thì tự hạ xuống hành vi `OnDemand`. Không có nó, paste một
/// file JSON lớn với `RunMode::Live` sẽ treo UI.
const LARGE_INPUT: usize = 256 * 1024;

pub struct ToolFormComponent {
    tool_id: Option<&'static str>,
    widgets: Vec<Box<dyn FieldWidget>>,
    /// Số widget đầu tiên là input + option; phần còn lại là output (chỉ đọc).
    editable_count: usize,
    focus: usize,
    mode: RunMode,
    error: Option<String>,
    run_at: Option<Instant>,
    focused: bool,
    queue: Queue,
    theme: SharedTheme,
    key_config: KeyConfig,
}

impl ToolFormComponent {
    pub fn new(queue: Queue, theme: SharedTheme, key_config: KeyConfig) -> Self {
        Self {
            tool_id: None,
            widgets: Vec::new(),
            editable_count: 0,
            focus: 0,
            mode: RunMode::Live,
            error: None,
            run_at: None,
            focused: false,
            queue,
            theme,
            key_config,
        }
    }

    /// Dựng lại toàn bộ widget từ spec và nạp `default` vào state.
    pub fn set_tool(&mut self, spec: &ToolSpec) {
        let mut widgets: Vec<Box<dyn FieldWidget>> = Vec::new();

        for field in spec.inputs.iter().chain(spec.options.iter()) {
            let mut w = build(field, self.theme.clone(), false);
            if let Some(default) = &field.default {
                w.set_value(default);
            }
            widgets.push(w);
        }
        let editable_count = widgets.len();

        for field in &spec.outputs {
            widgets.push(build(field, self.theme.clone(), true));
        }

        self.tool_id = Some(spec.id);
        self.widgets = widgets;
        self.editable_count = editable_count;
        self.focus = 0;
        self.mode = spec.mode;
        self.error = None;
        // Chạy ngay một lần để output không trống khi vừa mở tool.
        self.run_at = Some(Instant::now());
    }

    pub fn tool_id(&self) -> Option<&'static str> {
        self.tool_id
    }

    /// Tổng độ dài input — cơ sở cho ngưỡng an toàn 256KB.
    fn total_input_len(&self) -> usize {
        self.widgets
            .iter()
            .take(self.editable_count)
            .map(|w| w.value().as_display().len())
            .sum()
    }

    /// `RunMode` thực tế: hạ xuống `OnDemand` khi input quá lớn.
    fn effective_mode(&self) -> RunMode {
        if self.total_input_len() > LARGE_INPUT {
            RunMode::OnDemand
        } else {
            self.mode
        }
    }

    fn is_downgraded(&self) -> bool {
        self.mode == RunMode::Live && self.effective_mode() == RunMode::OnDemand
    }

    pub fn mark_dirty(&mut self) {
        if self.effective_mode() == RunMode::Live {
            self.run_at = Some(Instant::now() + DEBOUNCE);
        }
    }

    pub fn request_run_now(&mut self) {
        self.run_at = Some(Instant::now());
    }

    /// `true` khi tới hạn debounce — `App` gọi rồi chạy tool.
    pub fn take_run_request(&mut self) -> bool {
        match self.run_at {
            Some(at) if Instant::now() >= at => {
                self.run_at = None;
                true
            }
            _ => false,
        }
    }

    pub fn inputs(&self) -> Inputs {
        let mut inputs = Inputs::new();
        for w in self.widgets.iter().take(self.editable_count) {
            inputs.set(w.key(), w.value());
        }
        inputs
    }

    /// Nạp kết quả chạy tool vào form.
    ///
    /// `InvalidInput` mang tên field nên lỗi hiện **inline ngay dưới ô sai**;
    /// các lỗi khác hiện ở vùng lỗi chung.
    pub fn set_result(&mut self, result: Result<Outputs, ToolError>) {
        for w in &mut self.widgets {
            w.set_error(None);
        }
        self.error = None;

        match result {
            Ok(outputs) => {
                for w in self.widgets.iter_mut().skip(self.editable_count) {
                    if let Some(v) = outputs.get(w.key()) {
                        w.set_value(v);
                    }
                }
            }
            Err(ToolError::InvalidInput { field, msg }) => {
                match self.widgets.iter_mut().find(|w| w.key() == field) {
                    Some(w) => w.set_error(Some(msg)),
                    None => self.error = Some(format!("{field}: {msg}")),
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// Giá trị của widget đang focus — P3 dùng cho phím copy.
    pub fn focused_value(&self) -> Option<String> {
        self.widgets.get(self.focus).map(|w| w.value().as_display())
    }

    /// Nạp nội dung file vào **input chính** (field input đầu tiên của spec).
    /// Tool vẫn chỉ nhận text thuần — đọc file là việc của tầng UI.
    pub fn set_primary_input(&mut self, text: &str) {
        if let Some(w) = self.widgets.first_mut() {
            w.set_value(&Value::Text(text.to_string()));
            self.mark_dirty();
            self.request_run_now();
        }
    }
}

impl DrawableComponent for ToolFormComponent {
    fn draw(&self, f: &mut Frame, rect: Rect) -> Result<()> {
        if self.widgets.is_empty() {
            f.render_widget(
                Paragraph::new("Chọn một tool ở sidebar.").style(self.theme.dim()),
                rect,
            );
            return Ok(());
        }

        let mut y = rect.y;
        let bottom = rect.y + rect.height;

        for (i, w) in self.widgets.iter().enumerate() {
            let height = w.desired_height();
            if y >= bottom {
                break;
            }
            let area = Rect {
                x: rect.x,
                y,
                width: rect.width,
                height: height.min(bottom - y),
            };
            w.draw(f, area, self.focused && i == self.focus);
            y += area.height;
        }

        // Badge khi input lớn khiến tool tự hạ xuống chạy-theo-yêu-cầu.
        if self.is_downgraded() && y < bottom {
            f.render_widget(
                Paragraph::new(Line::from(format!(
                    "input lớn — nhấn {} để chạy",
                    self.key_config.hint(self.key_config.keys.confirm)
                )))
                .style(self.theme.dim()),
                Rect {
                    x: rect.x,
                    y,
                    width: rect.width,
                    height: 1,
                },
            );
            y += 1;
        }

        if let Some(err) = &self.error
            && y < bottom
        {
            f.render_widget(
                Paragraph::new(err.as_str())
                    .style(self.theme.error())
                    .wrap(Wrap { trim: false })
                    .block(
                        Block::bordered()
                            .border_style(self.theme.error())
                            .title(" Lỗi "),
                    ),
                Rect {
                    x: rect.x,
                    y,
                    width: rect.width,
                    height: (bottom - y).min(4),
                },
            );
        }
        Ok(())
    }
}

impl Component for ToolFormComponent {
    fn commands(&self, out: &mut Vec<CommandInfo>, force_all: bool) -> CommandBlocking {
        if (self.focused && !self.widgets.is_empty()) || force_all {
            let keys = &self.key_config.keys;
            out.push(
                CommandInfo::new(self.key_config.hint(keys.focus_next), "field kế", "Form")
                    .order(2),
            );
            if self.effective_mode() == RunMode::OnDemand {
                out.push(
                    CommandInfo::new(self.key_config.hint(keys.confirm), "chạy", "Form").order(3),
                );
            }
        }
        CommandBlocking::PassingOn
    }

    fn event(&mut self, ev: &Event) -> Result<EventState> {
        if !self.focused || self.widgets.is_empty() {
            return Ok(EventState::NotConsumed);
        }

        if let Event::Key(k) = ev {
            let keys = &self.key_config.keys;

            // Tab đi tới field kế; hết field thì trả quyền cho App để về sidebar.
            if key_match(k, keys.focus_next) {
                if self.focus + 1 < self.widgets.len() {
                    self.focus += 1;
                    return Ok(EventState::Consumed);
                }
                self.focus = 0;
                return Ok(EventState::NotConsumed);
            }

            // `OnDemand` (hoặc Live đã bị hạ cấp) chạy khi nhấn Enter.
            let on_demand = self.effective_mode() == RunMode::OnDemand;
            let editable = self.focus < self.editable_count;
            if on_demand && editable && key_match(k, keys.confirm) {
                self.queue.push(InternalEvent::RunRequested);
                return Ok(EventState::Consumed);
            }
        }

        let before = self.widgets[self.focus].value();
        let state = self.widgets[self.focus].event(ev, &self.key_config)?;

        if state.is_consumed() && self.widgets[self.focus].value() != before {
            self.queue.push(InternalEvent::InputChanged);
        }
        Ok(state)
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}
