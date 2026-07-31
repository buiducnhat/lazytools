//! Snapshot test qua `TestBackend` — không cần terminal thật.
//! Lần đầu chạy sinh `.snap.new`; duyệt bằng `cargo insta review`.

use lazytools::app::App;
use lazytools_core::registry::Registry;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn render(width: u16, height: u16) -> String {
    let mut terminal =
        Terminal::new(TestBackend::new(width, height)).expect("TestBackend phải dựng được");
    let mut app = App::new(Registry::new());

    terminal
        .draw(|f| app.draw(f).expect("draw không được lỗi"))
        .expect("draw phải chạy được");

    terminal.backend().to_string()
}

#[test]
fn layout_default_100x30() {
    insta::assert_snapshot!(render(100, 30));
}

/// Mốc responsive: >=80 cols sidebar đầy đủ.
#[test]
fn layout_wide_120_cols() {
    insta::assert_snapshot!(render(120, 24));
}

/// 60..80 cols → sidebar thu còn icon-only.
#[test]
fn layout_narrow_70_cols() {
    insta::assert_snapshot!(render(70, 20));
}

/// <60 cols → sidebar ẩn hẳn, workspace chiếm toàn bộ.
#[test]
fn layout_tiny_50_cols() {
    insta::assert_snapshot!(render(50, 16));
}
