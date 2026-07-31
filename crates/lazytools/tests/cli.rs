//! Test CLI end-to-end. Lưu ý về newline: stdout ở đây là pipe (không phải TTY),
//! nên output một-giá-trị là **raw bytes, không newline** — đúng thứ pipeline cần.

use assert_cmd::Command;

fn lazytools() -> Command {
    Command::cargo_bin("lazytools").expect("binary `lazytools` phải build được")
}

#[test]
fn hash_reads_stdin() {
    lazytools()
        .args(["hash", "--algo", "md5"])
        .write_stdin("hello world")
        .assert()
        .success()
        .stdout("5eb63bbbe01eeed093cb22bb8f5acdc3");
}

#[test]
fn hash_accepts_positional_arg() {
    lazytools()
        .args(["hash", "--algo", "md5", "hello world"])
        .assert()
        .success()
        .stdout("5eb63bbbe01eeed093cb22bb8f5acdc3");
}

#[test]
fn hash_sha256() {
    lazytools()
        .args(["hash", "--algo", "sha256", "hello world"])
        .assert()
        .success()
        .stdout("b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");
}

/// `FieldKind::Select` sinh `value_parser`, nên clap chặn giá trị lạ **trước khi**
/// tool chạy: exit 2 (mã lỗi dùng-sai-lệnh chuẩn của clap) kèm danh sách giá trị hợp lệ.
/// Nhánh `InvalidInput` → exit 1 của tầng tool được phủ ở P4, nơi có ca thật sự
/// tới được `run()` (ví dụ `hex --direction decode` với input hỏng).
#[test]
fn invalid_select_value_is_rejected_with_valid_choices() {
    let out = lazytools()
        .args(["hash", "--algo", "bogus", "x"])
        .assert()
        .code(2)
        .get_output()
        .clone();

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("algo"),
        "stderr phải nêu tên field: {stderr}"
    );
    assert!(
        stderr.contains("md5") && stderr.contains("sha256"),
        "stderr phải liệt kê giá trị hợp lệ: {stderr}"
    );
}

#[test]
fn help_lists_subcommands_from_registry() {
    let out = lazytools()
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .clone();

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("hash"),
        "--help phải liệt kê `hash`: {stdout}"
    );
}
