//! Snapshot test qua `TestBackend` — không cần terminal thật.
//! Lần đầu chạy sinh `.snap.new`; duyệt bằng `cargo insta review`.

use std::time::Duration;

use lazytools::app::App;
use lazytools_core::registry::Registry;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn terminal(width: u16, height: u16) -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(width, height)).expect("TestBackend phải dựng được")
}

fn draw(terminal: &mut Terminal<TestBackend>, app: &mut App) -> String {
    terminal
        .draw(|f| app.draw(f).expect("draw không được lỗi"))
        .expect("draw phải chạy được");
    terminal.backend().to_string()
}

fn render(width: u16, height: u16) -> String {
    let mut terminal = terminal(width, height);
    let mut app = App::new(Registry::new());
    draw(&mut terminal, &mut app)
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

/// End-to-end trong TUI: gõ input → debounce → `registry.run()` → digest hiện ra.
/// Giá trị phải **khớp chính xác** kết quả CLI cho cùng input.
#[test]
fn tool_form_shows_digest_matching_cli() {
    let mut terminal = terminal(90, 26);
    let mut app = App::new(Registry::new());

    // Tab: sidebar → workspace, focus rơi vào ô Input.
    app.event(&key(KeyCode::Tab)).expect("tab");
    app.process_queue().expect("queue");

    for c in "hello world".chars() {
        app.event(&key(KeyCode::Char(c))).expect("gõ ký tự");
    }
    app.process_queue().expect("queue");

    // Đợi qua ngưỡng debounce 80ms rồi để `tick()` chạy tool.
    std::thread::sleep(Duration::from_millis(150));
    app.tick();

    let screen = draw(&mut terminal, &mut app);

    assert!(
        screen.contains("5eb63bbbe01eeed093cb22bb8f5acdc3"),
        "digest md5 của \"hello world\" phải hiện trong form:\n{screen}"
    );
    insta::assert_snapshot!(screen);
}

/// Ngưỡng an toàn: input > 256KB thì `RunMode::Live` tự hạ xuống chạy-theo-yêu-cầu
/// kèm badge, để paste một file lớn không treo UI.
#[test]
fn large_input_downgrades_to_on_demand() {
    let mut terminal = terminal(90, 30);
    let mut app = App::new(Registry::new());

    app.event(&key(KeyCode::Tab)).expect("tab");
    app.process_queue().expect("queue");

    app.event(&Event::Paste("a".repeat(300 * 1024)))
        .expect("paste khối lớn");
    app.process_queue().expect("queue");

    // Quá ngưỡng debounce mà vẫn không tự chạy — đó chính là hành vi mong muốn.
    std::thread::sleep(Duration::from_millis(150));
    app.tick();
    let screen = draw(&mut terminal, &mut app);
    assert!(
        screen.contains("input lớn"),
        "phải hiện badge nhắc nhấn Enter:\n{screen}"
    );

    // Nhấn Enter thì mới chạy, và chạy đúng.
    app.event(&key(KeyCode::Enter)).expect("enter");
    app.process_queue().expect("queue");
    app.tick();
    let screen = draw(&mut terminal, &mut app);

    // md5 của 307200 ký tự 'a'.
    let expected = {
        use md5::{Digest, Md5};
        hex::encode(Md5::digest("a".repeat(300 * 1024).as_bytes()))
    };
    assert!(
        screen.contains(&expected),
        "sau khi nhấn Enter phải có digest {expected}:\n{screen}"
    );
}
