//! The body of `lazytools` lives in the lib target so `tests/` can reach `App` and
//! the components — integration tests can't import from a binary crate.

pub mod app;
pub mod cli;
pub mod clipboard;
pub mod components;
pub mod keys;
pub mod popups;
pub mod queue;
pub mod tui;
pub mod ui;
