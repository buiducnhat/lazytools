//! A generic component that reads a `ToolSpec` and builds the form. No tool name lives here —
//! that's why adding the 9th tool costs zero lines of UI code.

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

/// How long to wait after typing stops before re-running the tool.
const DEBOUNCE: Duration = Duration::from_millis(80);

/// Above this threshold, behavior auto-downgrades to `OnDemand`. Without it, pasting a
/// large JSON file with `RunMode::Live` would freeze the UI.
const LARGE_INPUT: usize = 256 * 1024;

pub struct ToolFormComponent {
    tool_id: Option<&'static str>,
    widgets: Vec<Box<dyn FieldWidget>>,
    /// The first N widgets are inputs + options; the rest are outputs (read-only).
    editable_count: usize,
    /// How many of those leading widgets are *inputs*. Zero for generators, which is
    /// what stops "open file" from writing into the first option.
    input_count: usize,
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
            input_count: 0,
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

    /// Rebuilds all widgets from the spec and loads `default` into state.
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
        self.input_count = spec.inputs.len();
        self.focus = 0;
        self.mode = spec.mode;
        self.error = None;
        self.run_at = None;
        self.autorun();
    }

    pub fn tool_id(&self) -> Option<&'static str> {
        self.tool_id
    }

    /// Total input length — the basis for the 256KB safety threshold.
    fn total_input_len(&self) -> usize {
        self.widgets
            .iter()
            .take(self.editable_count)
            .map(|w| w.value().as_display().len())
            .sum()
    }

    /// The effective `RunMode`: downgraded to `OnDemand` when input is too large.
    fn effective_mode(&self) -> RunMode {
        if self.total_input_len() > LARGE_INPUT {
            RunMode::OnDemand
        } else {
            self.mode
        }
    }

    /// Deliberately only about `Live`: a `Generate` tool has nothing but small options,
    /// so it can never reach the 256KB threshold and never gets downgraded.
    fn is_downgraded(&self) -> bool {
        self.mode == RunMode::Live && self.effective_mode() == RunMode::OnDemand
    }

    /// Whether the tool runs by itself, without waiting for a key press.
    fn runs_automatically(&self) -> bool {
        matches!(self.effective_mode(), RunMode::Live | RunMode::Generate)
    }

    /// Runs the tool right away, but **only** if it runs automatically. `OnDemand` tools
    /// stay put: bcrypt at cost 12 costs ~200ms, so auto-running it on open would freeze
    /// the UI to hash an empty password nobody asked for. That's the whole reason
    /// `OnDemand` exists.
    fn autorun(&mut self) {
        if self.runs_automatically() {
            self.run_at = Some(Instant::now());
        }
    }

    pub fn mark_dirty(&mut self) {
        if self.runs_automatically() {
            self.run_at = Some(Instant::now() + DEBOUNCE);
        }
    }

    pub fn request_run_now(&mut self) {
        self.run_at = Some(Instant::now());
    }

    /// `true` once the debounce deadline is reached — `App` calls this then runs the tool.
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

    /// Loads the tool run result into the form.
    ///
    /// `InvalidInput` carries a field name so the error shows **inline right under the
    /// offending field**; other errors show in the general error area.
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

    /// Value of the currently focused widget — used by P3 for the copy key.
    pub fn focused_value(&self) -> Option<String> {
        self.widgets.get(self.focus).map(|w| w.value().as_display())
    }

    /// Whether "open file" has anywhere to put the content. A tool with no inputs
    /// (every generator) has none — the first widget there is an *option*.
    pub fn accepts_file_input(&self) -> bool {
        self.input_count > 0
    }

    /// Loads file content into the **primary input** (the spec's first input field).
    /// The tool still only receives plain text — reading the file is the UI layer's job.
    pub fn set_primary_input(&mut self, text: &str) {
        // Guard, not an `if let`: without an input, `widgets.first_mut()` is the first
        // option, and this would dump a whole file into e.g. a `length` box.
        if !self.accepts_file_input() {
            return;
        }
        if let Some(w) = self.widgets.first_mut() {
            w.set_value(&Value::Text(text.to_string()));
            self.autorun();
        }
    }
}

impl DrawableComponent for ToolFormComponent {
    fn draw(&self, f: &mut Frame, rect: Rect) -> Result<()> {
        if self.widgets.is_empty() {
            f.render_widget(
                Paragraph::new("Select a tool in the sidebar.").style(self.theme.dim()),
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

        // Badge shown whenever the confirm key does something — either the tool won't run
        // on its own (`OnDemand`, or large input auto-downgraded it), or it will produce a
        // fresh result (`Generate`). Without it, an `OnDemand` tool just shows an empty
        // output with no clue that a keypress is what's missing.
        // The run key rather than confirm: it is the one that works from *every*
        // field. Naming `Enter` here would be a lie the moment focus sits on a
        // multiline input, and a hint that drifts from behavior is worse than none.
        let key = self.key_config.hint(self.key_config.keys.run);
        let badge = match self.effective_mode() {
            RunMode::OnDemand => {
                let why = if self.is_downgraded() {
                    "large input — "
                } else {
                    ""
                };
                Some(format!("{why}press {key} to run"))
            }
            RunMode::Generate => Some(format!("press {key} to regenerate")),
            RunMode::Live => None,
        };
        if let Some(badge) = badge
            && y < bottom
        {
            f.render_widget(
                Paragraph::new(Line::from(badge)).style(self.theme.dim()),
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
                            .title(" Error "),
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
                CommandInfo::new(self.key_config.hint(keys.focus_next), "next field", "Form")
                    .order(2),
            );
            let action = match self.effective_mode() {
                RunMode::OnDemand => Some("run"),
                RunMode::Generate => Some("regenerate"),
                RunMode::Live => None,
            };
            if let Some(action) = action {
                out.push(CommandInfo::new(self.key_config.hint(keys.run), action, "Form").order(3));
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

            // Tab moves to the next field; once fields run out, control returns to App to go back to the sidebar.
            if key_match(k, keys.focus_next) {
                if self.focus + 1 < self.widgets.len() {
                    self.focus += 1;
                    return Ok(EventState::Consumed);
                }
                self.focus = 0;
                return Ok(EventState::NotConsumed);
            }

            // `OnDemand` (or Live that's been downgraded) runs on request; `Generate`
            // re-runs to produce a fresh value.
            //
            // The run key works from any field, outputs included. The confirm key also
            // runs the tool, but only where it isn't already spoken for: a multiline
            // text field needs `Enter` for line breaks. Giving the focused widget first
            // refusal is what stops the 256KB downgrade from silently turning `Enter`
            // into "run" inside an editable multiline field — the input still being
            // edited is exactly the one that loses its line breaks.
            let runnable = matches!(self.effective_mode(), RunMode::OnDemand | RunMode::Generate);
            let editable = self.focus < self.editable_count;
            let field_wants_confirm = self.widgets[self.focus].wants_confirm_key();
            let requested = key_match(k, keys.run)
                || (editable && !field_wants_confirm && key_match(k, keys.confirm));
            if runnable && requested {
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
