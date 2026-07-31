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

// --- Mỗi tool thêm ở P4 có ít nhất một ca, tập trung vào luồng pipe. ---

#[test]
fn base64_encode_from_stdin() {
    lazytools()
        .arg("base64")
        .write_stdin("hello")
        .assert()
        .success()
        .stdout("aGVsbG8=");
}

#[test]
fn base64_decode() {
    lazytools()
        .args(["base64", "--direction", "decode"])
        .write_stdin("aGVsbG8=")
        .assert()
        .success()
        .stdout("hello");
}

#[test]
fn hex_encode() {
    lazytools()
        .arg("hex")
        .write_stdin("hello")
        .assert()
        .success()
        .stdout("68656c6c6f");
}

/// Nhánh `InvalidInput` → exit **1** của tầng tool, hoãn từ P1 vì lúc đó chưa
/// có ca nào tới được `run()` (clap chặn hết ở tầng `Select`).
#[test]
fn tool_level_invalid_input_exits_1_and_names_the_field() {
    let out = lazytools()
        .args(["hex", "--direction", "decode", "zzz"])
        .assert()
        .code(1)
        .get_output()
        .clone();

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.starts_with("error: text:"),
        "positional phải in `text:` chứ không phải `--text:`: {stderr}"
    );
}

#[test]
fn url_encode() {
    lazytools()
        .arg("url")
        .write_stdin("hello world")
        .assert()
        .success()
        .stdout("hello%20world");
}

#[test]
fn hmac_sha256_with_key() {
    lazytools()
        .args(["hmac", "--algo", "sha256", "--key", "key"])
        .write_stdin("The quick brown fox jumps over the lazy dog")
        .assert()
        .success()
        .stdout("f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8");
}

#[test]
fn bcrypt_hash_then_verify() {
    let out = lazytools()
        .args(["bcrypt", "--cost", "4", "hunter2"])
        .assert()
        .success()
        .get_output()
        .clone();
    let hash = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(hash.starts_with("$2"), "phải là chuỗi bcrypt: {hash}");

    lazytools()
        .args(["bcrypt", "--mode", "verify", "--hash", &hash, "hunter2"])
        .assert()
        .success()
        .stdout("true");
}

#[test]
fn json_format_minify() {
    lazytools()
        .args(["json-format", "--mode", "minify"])
        .write_stdin("{ \"a\" : 1 }")
        .assert()
        .success()
        .stdout(r#"{"a":1}"#);
}

/// Khóa phải giữ **đúng thứ tự người dùng viết**, không bị sắp lại theo bảng chữ cái.
#[test]
fn json_format_preserves_key_order() {
    lazytools()
        .args(["json-format", "--mode", "minify"])
        .write_stdin(r#"{"zebra":1,"apple":2}"#)
        .assert()
        .success()
        .stdout(r#"{"zebra":1,"apple":2}"#);
}

#[test]
fn data_format_json_to_yaml() {
    lazytools()
        .args(["data-format", "--from", "json", "--to", "yaml"])
        .write_stdin(r#"{"a":1}"#)
        .assert()
        .success()
        .stdout("a: 1\n");
}

/// Giới hạn thật của TOML phải thành lỗi rõ ràng, không phải output sai âm thầm.
#[test]
fn data_format_reports_toml_limits() {
    let out = lazytools()
        .args(["data-format", "--from", "json", "--to", "toml"])
        .write_stdin("[1,2]")
        .assert()
        .code(1)
        .get_output()
        .clone();

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--to:"),
        "lỗi trên option phải in kèm `--to`: {stderr}"
    );
}

#[test]
fn help_lists_all_eight_tools() {
    let out = lazytools()
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&out.stdout);

    for name in [
        "hash",
        "hmac",
        "bcrypt",
        "base64",
        "url",
        "hex",
        "json-format",
        "data-format",
    ] {
        assert!(stdout.contains(name), "--help thiếu `{name}`:\n{stdout}");
    }
}
