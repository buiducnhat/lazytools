//! Snapshot test qua `TestBackend` — không cần terminal thật.
//! Lần đầu chạy sinh `.snap.new`; duyệt bằng `cargo insta review`.

use std::time::Duration;

use lazytools::app::App;
use lazytools_core::error::ToolError;
use lazytools_core::registry::{Registry, Tool};
use lazytools_core::spec::{Category, Field, ToolSpec};
use lazytools_core::value::{Inputs, Outputs};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn ctrl(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::CONTROL))
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

/// Palette mở bằng `Ctrl+P`, gõ `md5` phải đưa Hash Text lên đầu.
#[test]
fn palette_matches_on_keywords() {
    let mut terminal = terminal(80, 24);
    let mut app = App::new(Registry::new());

    app.event(&ctrl(KeyCode::Char('p'))).expect("ctrl+p");
    app.process_queue().expect("queue");
    for c in "md5".chars() {
        app.event(&key(KeyCode::Char(c))).expect("gõ");
    }
    app.process_queue().expect("queue");

    let screen = draw(&mut terminal, &mut app);
    assert!(
        screen.contains("Tìm tool") && screen.contains("Hash Text"),
        "palette phải mở và khớp `md5` → Hash Text (khớp qua keywords):\n{screen}"
    );
    insta::assert_snapshot!(screen);
}

/// Help popup dựng từ `commands()` — phải chứa đúng chuỗi phím của `KeyConfig`.
#[test]
fn help_popup_is_generated_from_commands() {
    let mut terminal = terminal(80, 24);
    let mut app = App::new(Registry::new());

    app.event(&key(KeyCode::Char('?'))).expect("?");
    app.process_queue().expect("queue");

    let screen = draw(&mut terminal, &mut app);
    assert!(
        screen.contains("Phím tắt") && screen.contains("^P") && screen.contains("thoát"),
        "help phải liệt kê phím sinh từ commands():\n{screen}"
    );
    insta::assert_snapshot!(screen);
}

/// Tool chỉ dùng trong test: phủ **mọi** biến thể `FieldKind` để chứng minh
/// không biến thể nào thiếu widget, và `Secret` thật sự được che.
struct AllKindsTool {
    spec: ToolSpec,
}

impl Default for AllKindsTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("text.allkinds", "All Kinds", Category::Text)
                .describe("phủ mọi FieldKind")
                .input(Field::text("text").label("Text"))
                .option(Field::secret("key").label("Secret"))
                .option(Field::number("cost", 4, 15).default(12i64).label("Number"))
                .option(Field::toggle("flag").label("Toggle"))
                .option(
                    Field::select("mode", &["a", "b"])
                        .default("a")
                        .label("Select"),
                )
                .option(Field::filepath("path", false).label("FilePath"))
                .output(Field::text("result").label("Result")),
        }
    }
}

impl Tool for AllKindsTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }
    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        Ok(Outputs::one("result", i.text("text").to_uppercase()))
    }
}

#[test]
fn every_field_kind_renders_and_secret_is_masked() {
    let registry = Registry::from_tools(vec![Box::new(AllKindsTool::default())]);
    let mut terminal = terminal(80, 34);
    let mut app = App::new(registry);

    // Vào form, gõ vào ô Text, rồi Tab sang ô Secret và gõ khóa.
    app.event(&key(KeyCode::Tab)).expect("tab");
    app.process_queue().expect("queue");
    for c in "hi".chars() {
        app.event(&key(KeyCode::Char(c))).expect("gõ text");
    }
    app.event(&key(KeyCode::Tab)).expect("tab sang secret");
    app.process_queue().expect("queue");
    for c in "s3cret".chars() {
        app.event(&key(KeyCode::Char(c))).expect("gõ secret");
    }
    app.process_queue().expect("queue");
    std::thread::sleep(Duration::from_millis(150));
    app.tick();

    let screen = draw(&mut terminal, &mut app);

    // Mọi FieldKind đều có mặt trên màn hình.
    for label in ["Text", "Secret", "Number", "Toggle", "Select", "FilePath"] {
        assert!(
            screen.contains(label),
            "thiếu widget cho {label}:\n{screen}"
        );
    }
    // Secret bị che, giá trị thật không bao giờ lộ ra buffer.
    assert!(
        !screen.contains("s3cret"),
        "giá trị Secret KHÔNG được xuất hiện trên màn hình:\n{screen}"
    );
    assert!(
        screen.contains("••••••"),
        "Secret phải render dạng che:\n{screen}"
    );

    insta::assert_snapshot!(screen);
}

/// `y` là phím copy ở cấp app, nhưng trong ô nhập nó phải là ký tự bình thường.
/// Ô output là chỉ-đọc nên không tiêu thụ phím, và `y` mới rơi xuống app để copy —
/// đó là lý do định tuyến "widget trước, app sau" là đúng thứ tự.
#[test]
fn copy_key_still_types_inside_an_editable_field() {
    let mut terminal = terminal(90, 26);
    let mut app = App::new(Registry::new());

    app.event(&key(KeyCode::Tab)).expect("tab vào form");
    app.process_queue().expect("queue");
    for c in "yes".chars() {
        app.event(&key(KeyCode::Char(c))).expect("gõ");
    }
    app.process_queue().expect("queue");
    std::thread::sleep(Duration::from_millis(150));
    app.tick();

    let screen = draw(&mut terminal, &mut app);
    assert!(
        screen.contains("yes"),
        "`y` trong ô nhập phải là ký tự, không phải lệnh copy:\n{screen}"
    );
    // md5("yes")
    assert!(
        screen.contains("a6105c0a611b41b08f1209506350279e"),
        "tool phải chạy trên đúng chuỗi đã gõ:\n{screen}"
    );
}
