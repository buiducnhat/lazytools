//! Thân của `lazytools` nằm ở lib target để `tests/` chạm được vào `App` và các
//! component — integration test không import được từ binary crate.

pub mod app;
pub mod cli;
pub mod clipboard;
pub mod components;
pub mod keys;
pub mod popups;
pub mod queue;
pub mod tui;
pub mod ui;
