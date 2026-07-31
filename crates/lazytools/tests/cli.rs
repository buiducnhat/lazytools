//! End-to-end CLI tests. Note on newlines: stdout here is a pipe (not a TTY),
//! so single-value output is **raw bytes, no newline** — exactly what pipelines need.

use assert_cmd::Command;

fn lazytools() -> Command {
    Command::cargo_bin("lazytools").expect("binary `lazytools` must build")
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

/// `FieldKind::Select` generates a `value_parser`, so clap rejects an unknown value
/// **before** the tool runs: exit 2 (clap's standard usage-error code) with the list
/// of valid values. The `InvalidInput` → exit 1 branch of the tool layer is covered
/// in P4, where a case actually reaches `run()` (e.g. `hex --direction decode` with
/// malformed input).
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
        "stderr must name the field: {stderr}"
    );
    assert!(
        stderr.contains("md5") && stderr.contains("sha256"),
        "stderr must list the valid values: {stderr}"
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
    assert!(stdout.contains("hash"), "--help must list `hash`: {stdout}");
}

// --- Every tool added in P4 has at least one case, focused on the pipe flow. ---

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

/// The `InvalidInput` → exit **1** branch of the tool layer, deferred from P1 since
/// at that point no case reached `run()` yet (clap rejected everything at the
/// `Select` layer).
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
        "the positional arg must print `text:`, not `--text:`: {stderr}"
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
    assert!(hash.starts_with("$2"), "must be a bcrypt string: {hash}");

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

/// Keys must keep **exactly the order the user wrote them in**, not get resorted alphabetically.
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

/// A real TOML limitation must become a clear error, not silently-wrong output.
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
        "the error on the option must include `--to`: {stderr}"
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
        assert!(
            stdout.contains(name),
            "--help is missing `{name}`:\n{stdout}"
        );
    }
}
