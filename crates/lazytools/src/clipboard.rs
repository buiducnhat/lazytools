//! Copy to the system clipboard, with an OSC 52 fallback for remote sessions.
//!
//! Two backends:
//!
//! - **Native** (`arboard`) — talks to the clipboard of the machine the process
//!   runs on.
//! - **OSC 52** — writes an escape sequence to the terminal, asking the
//!   *terminal emulator* to set its clipboard. Over SSH that is the machine the
//!   user is sitting at, which is the one they meant.
//!
//! Neither is a strict improvement on the other, so which one is tried first
//! depends on the session (see [`prefers_terminal`]). Failure must be
//! **reported clearly** — no panic, no silent failure.

use std::io::Write;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

/// Which backend actually took the text — the UI says so, because the two put it
/// in different places and the user is the only one who knows which they wanted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// The clipboard of the machine this process runs on.
    Native,
    /// The terminal emulator's clipboard, via an OSC 52 escape sequence.
    Osc52,
}

impl Backend {
    /// Flash text for the command bar.
    pub fn flash(self) -> &'static str {
        match self {
            Self::Native => "copied",
            Self::Osc52 => "copied to terminal clipboard (OSC 52)",
        }
    }
}

/// Cap on the text handed to OSC 52.
///
/// The sequence has no reply and no acknowledgement, so a terminal that refuses
/// an over-long string simply drops it — and a clipboard silently holding half a
/// document is worse than a copy that says it didn't happen. Terminals vary
/// (xterm's limit is a build-time constant, tmux's `set-clipboard` buffer is its
/// own), so this is a conservative floor rather than any one implementation's
/// number. Larger text has a better path anyway: save it to a file.
const MAX_OSC52_BYTES: usize = 64 * 1024;

/// Copies `text`, returning the backend that accepted it.
///
/// `Err` carries a reason the user can read, listing what was tried.
pub fn copy(text: &str) -> Result<Backend, String> {
    let mux = Multiplexer::detect();
    if prefers_terminal() {
        // Over SSH the native clipboard is the *remote* machine's. When one
        // exists (an X server on the remote host), writing to it "succeeds"
        // while putting the text somewhere the user can't paste from — so the
        // terminal gets first refusal here, not second.
        match copy_osc52(text, mux) {
            Ok(()) => Ok(Backend::Osc52),
            Err(terminal_err) => copy_native(text)
                .map(|()| Backend::Native)
                .map_err(|native_err| format!("{terminal_err}\n\nAlso tried: {native_err}")),
        }
    } else {
        match copy_native(text) {
            Ok(()) => Ok(Backend::Native),
            Err(native_err) => copy_osc52(text, mux)
                .map(|()| Backend::Osc52)
                .map_err(|terminal_err| format!("{native_err}\n\nAlso tried: {terminal_err}")),
        }
    }
}

/// True when the terminal emulator is likely on a different machine than this
/// process, so its clipboard is the one the user can paste from.
fn prefers_terminal() -> bool {
    ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"]
        .iter()
        .any(|k| std::env::var_os(k).is_some_and(|v| !v.is_empty()))
}

fn copy_native(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|e| format!("couldn't open the system clipboard: {e}"))?;
    clipboard
        .set_text(text.to_owned())
        .map_err(|e| format!("couldn't write to the system clipboard: {e}"))
}

fn copy_osc52(text: &str, mux: Multiplexer) -> Result<(), String> {
    let seq = osc52_sequence(text, mux)?;
    let mut out = std::io::stdout();
    out.write_all(seq.as_bytes())
        .and_then(|()| out.flush())
        .map_err(|e| format!("couldn't write to the terminal: {e}"))
}

/// Terminal multiplexer wrapping this process, if any.
///
/// A multiplexer sits between the program and the terminal emulator that owns
/// the clipboard, so the escape sequence has to be addressed to it explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Multiplexer {
    None,
    Tmux,
    Screen,
}

impl Multiplexer {
    fn detect() -> Self {
        if std::env::var_os("TMUX").is_some_and(|v| !v.is_empty()) {
            // tmux sets `TERM=screen*`/`tmux*` inside its panes, so this check
            // has to come first or every tmux session reads as GNU screen.
            Self::Tmux
        } else if std::env::var("TERM").is_ok_and(|t| t.starts_with("screen")) {
            Self::Screen
        } else {
            Self::None
        }
    }
}

/// Builds the escape sequence. Pure, and the multiplexer is a parameter rather
/// than an environment read, so every branch is testable.
fn osc52_sequence(text: &str, mux: Multiplexer) -> Result<String, String> {
    if text.len() > MAX_OSC52_BYTES {
        return Err(format!(
            "{} KB is too large for the terminal clipboard escape (limit {} KB).\n\
             Save it to a file instead.",
            text.len() / 1024,
            MAX_OSC52_BYTES / 1024
        ));
    }
    if mux == Multiplexer::Screen {
        // Unlike tmux, GNU screen has no passthrough that a program can switch
        // on for itself, and an unwrapped sequence is swallowed with no error.
        // Saying so beats writing bytes into a void and flashing "copied".
        return Err(
            "GNU screen does not forward the terminal clipboard escape (OSC 52).".to_string(),
        );
    }

    // `c` is the selection: the clipboard proper, not the X11 primary selection.
    let payload = STANDARD.encode(text);
    let inner = format!("\x1b]52;c;{payload}\x07");
    Ok(match mux {
        // tmux passes a DCS-wrapped sequence to the outer terminal only when
        // `allow-passthrough` is on, and it needs every ESC inside doubled.
        Multiplexer::Tmux => format!("\x1bPtmux;{}\x1b\\", inner.replace('\x1b', "\x1b\x1b")),
        _ => inner,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_sequence_is_osc52_with_a_base64_payload() {
        let seq = osc52_sequence("hi", Multiplexer::None).unwrap();
        assert_eq!(seq, "\x1b]52;c;aGk=\x07");
    }

    #[test]
    fn tmux_wraps_the_sequence_and_doubles_every_esc() {
        let seq = osc52_sequence("hi", Multiplexer::Tmux).unwrap();
        assert_eq!(seq, "\x1bPtmux;\x1b\x1b]52;c;aGk=\x07\x1b\\");
        // The wrapper's own ESCs must stay single, or tmux never sees the end.
        assert!(seq.starts_with("\x1bPtmux;"));
        assert!(seq.ends_with("\x1b\\"));
    }

    #[test]
    fn screen_is_refused_rather_than_written_into_a_void() {
        let err = osc52_sequence("hi", Multiplexer::Screen).unwrap_err();
        assert!(err.contains("screen"), "{err}");
    }

    /// Truncation is undetectable over OSC 52 — there is no reply — so the
    /// oversize case has to fail loudly before anything is written.
    #[test]
    fn oversize_text_is_refused_with_the_limit_stated() {
        let big = "x".repeat(MAX_OSC52_BYTES + 1);
        let err = osc52_sequence(&big, Multiplexer::None).unwrap_err();
        assert!(err.contains("64 KB"), "{err}");
        assert!(osc52_sequence(&"x".repeat(MAX_OSC52_BYTES), Multiplexer::None).is_ok());
    }

    #[test]
    fn non_ascii_survives_the_round_trip() {
        let seq = osc52_sequence("héllo ✓", Multiplexer::None).unwrap();
        let payload = seq
            .trim_start_matches("\x1b]52;c;")
            .trim_end_matches('\x07');
        assert_eq!(STANDARD.decode(payload).unwrap(), "héllo ✓".as_bytes());
    }
}
