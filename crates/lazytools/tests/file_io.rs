//! File I/O trong TUI. Các test này **ghi file thật**, nên chỉ đụng tới thư mục
//! tạm tự tạo — không bao giờ chạm file trong repo.

use std::path::PathBuf;

use lazytools::app::App;
use lazytools_core::registry::Registry;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn ctrl(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::CONTROL))
}

fn type_str(app: &mut App, text: &str) {
    for c in text.chars() {
        app.event(&key(KeyCode::Char(c))).expect("gõ ký tự");
    }
}

fn screen(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal =
        Terminal::new(TestBackend::new(width, height)).expect("TestBackend phải dựng được");
    terminal
        .draw(|f| app.draw(f).expect("draw không được lỗi"))
        .expect("draw phải chạy được");
    terminal.backend().to_string()
}

/// Thư mục rác riêng cho mỗi test, xoá khi xong.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("lazytools-test-{name}"));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("tạo được thư mục tạm");
        Self(path)
    }
    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// `Ctrl+S` mở popup lưu; nhập đường dẫn mới rồi Enter là ghi ngay (không hỏi).
#[test]
fn saves_focused_output_to_a_new_file() {
    let dir = TempDir::new("save-new");
    let target = dir.join("out.txt");

    let mut app = App::new(Registry::new());
    app.event(&key(KeyCode::Tab)).expect("vào form");
    app.process_queue().expect("queue");
    type_str(&mut app, "hello world");
    app.process_queue().expect("queue");
    std::thread::sleep(std::time::Duration::from_millis(150));
    app.tick();

    // Tab tới ô Digest (output) rồi lưu.
    app.event(&key(KeyCode::Tab)).expect("tab");
    app.event(&key(KeyCode::Tab)).expect("tab");
    app.process_queue().expect("queue");
    app.event(&ctrl(KeyCode::Char('s'))).expect("ctrl+s");
    app.process_queue().expect("queue");

    type_str(&mut app, target.to_str().unwrap());
    app.event(&key(KeyCode::Enter)).expect("enter");
    app.process_queue().expect("queue");

    let written = std::fs::read_to_string(&target).expect("file phải được tạo");
    assert_eq!(written, "5eb63bbbe01eeed093cb22bb8f5acdc3");
}

/// **Ghi đè luôn phải có bước xác nhận** — thao tác không thể hoàn tác duy nhất
/// trong toàn bộ app.
#[test]
fn overwrite_always_asks_first() {
    let dir = TempDir::new("save-overwrite");
    let target = dir.join("exists.txt");
    std::fs::write(&target, "NỘI DUNG CŨ").expect("tạo file sẵn có");

    let mut app = App::new(Registry::new());
    app.event(&key(KeyCode::Tab)).expect("vào form");
    app.process_queue().expect("queue");
    type_str(&mut app, "hello world");
    app.process_queue().expect("queue");
    std::thread::sleep(std::time::Duration::from_millis(150));
    app.tick();

    app.event(&key(KeyCode::Tab)).expect("tab");
    app.event(&key(KeyCode::Tab)).expect("tab");
    app.process_queue().expect("queue");
    app.event(&ctrl(KeyCode::Char('s'))).expect("ctrl+s");
    app.process_queue().expect("queue");
    type_str(&mut app, target.to_str().unwrap());
    app.event(&key(KeyCode::Enter)).expect("enter");
    app.process_queue().expect("queue");

    // Enter lần đầu chỉ mở bước xác nhận — file vẫn nguyên vẹn.
    let out = screen(&mut app, 100, 30);
    assert!(out.contains("ghi đè"), "phải hỏi xác nhận:\n{out}");
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "NỘI DUNG CŨ",
        "chưa xác nhận thì KHÔNG được ghi đè"
    );

    // Esc huỷ → vẫn giữ nguyên.
    app.event(&key(KeyCode::Esc)).expect("esc");
    app.process_queue().expect("queue");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "NỘI DUNG CŨ");

    // Enter → vào lại xác nhận, Enter nữa → mới thật sự ghi.
    app.event(&key(KeyCode::Enter)).expect("enter");
    app.process_queue().expect("queue");
    app.event(&key(KeyCode::Enter)).expect("xác nhận");
    app.process_queue().expect("queue");
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "5eb63bbbe01eeed093cb22bb8f5acdc3"
    );
}

/// Thư mục cha không tồn tại → báo lỗi, **không** tự tạo thư mục.
#[test]
fn missing_parent_directory_errors_without_creating_it() {
    let dir = TempDir::new("save-noparent");
    let missing = dir.join("khong-ton-tai");
    let target = missing.join("out.txt");

    let mut app = App::new(Registry::new());
    app.event(&key(KeyCode::Tab)).expect("vào form");
    app.process_queue().expect("queue");
    app.event(&key(KeyCode::Tab)).expect("tab");
    app.event(&key(KeyCode::Tab)).expect("tab");
    app.process_queue().expect("queue");
    app.event(&ctrl(KeyCode::Char('s'))).expect("ctrl+s");
    app.process_queue().expect("queue");
    type_str(&mut app, target.to_str().unwrap());
    app.event(&key(KeyCode::Enter)).expect("enter");
    app.process_queue().expect("queue");

    let out = screen(&mut app, 100, 30);
    assert!(out.contains("không tồn tại"), "phải báo lỗi rõ:\n{out}");
    assert!(!missing.exists(), "KHÔNG được tự tạo thư mục cha");
    assert!(!target.exists());
}

/// `Ctrl+O` mở picker, chọn file → nội dung nạp vào input chính và tool chạy lại.
#[test]
fn opens_a_file_into_the_primary_input() {
    let dir = TempDir::new("open-file");
    let file = dir.join("input.txt");
    std::fs::write(&file, "hello world").expect("ghi file mẫu");

    let mut app = App::new(Registry::new());
    // Đi đường tắt qua queue thay vì mô phỏng điều hướng thư mục —
    // phần duyệt thư mục đã được popup lo, ở đây kiểm luồng nạp vào form.
    app.event(&ctrl(KeyCode::Char('o'))).expect("ctrl+o");
    app.process_queue().expect("queue");
    let out = screen(&mut app, 100, 30);
    assert!(out.contains("Mở file"), "picker phải mở:\n{out}");

    // Đóng picker để overlay không che form khi kiểm kết quả.
    app.event(&key(KeyCode::Esc)).expect("esc");
    app.process_queue().expect("queue");

    app.open_file(&file);
    app.process_queue().expect("queue");
    std::thread::sleep(std::time::Duration::from_millis(150));
    app.tick();

    let out = screen(&mut app, 100, 30);
    assert!(
        out.contains("hello world"),
        "nội dung file phải vào input:\n{out}"
    );
    assert!(
        out.contains("5eb63bbbe01eeed093cb22bb8f5acdc3"),
        "tool phải chạy lại trên nội dung mới:\n{out}"
    );
}

/// File quá lớn bị từ chối kèm thông báo, không treo và không nạp.
#[test]
fn rejects_files_over_the_size_limit() {
    let dir = TempDir::new("open-toobig");
    let file = dir.join("big.txt");
    let limit = lazytools::popups::file_open::MAX_FILE_BYTES as usize;
    std::fs::write(&file, "a".repeat(limit + 1)).expect("ghi file lớn");

    let meta = std::fs::metadata(&file).unwrap();
    assert!(meta.len() > lazytools::popups::file_open::MAX_FILE_BYTES);

    let mut app = App::new(Registry::new());
    app.event(&ctrl(KeyCode::Char('o'))).expect("ctrl+o");
    app.process_queue().expect("queue");

    let rejected = lazytools::popups::file_open::check_openable(&file);
    assert!(rejected.is_err(), "file quá lớn phải bị từ chối");
    let msg = rejected.unwrap_err();
    assert!(
        msg.contains("giới hạn"),
        "thông báo phải nêu giới hạn: {msg}"
    );

    // Và đi qua App thì cũng không nạp vào form, chỉ hiện lỗi.
    app.open_file(&file);
    let out = screen(&mut app, 100, 30);
    assert!(out.contains("giới hạn"), "phải hiện lý do từ chối:\n{out}");
}
