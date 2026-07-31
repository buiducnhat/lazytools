//! Copy to the system clipboard.
//!
//! Failure (common when running over SSH: no clipboard server) must be
//! **reported clearly**, no panic, no silent failure. OSC52 fallback has been deferred to v0.2.

/// Returns `Err` with a reason the user can read.
pub fn copy(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| {
        format!("couldn't open clipboard: {e}\nOver SSH there's usually none available.")
    })?;
    clipboard
        .set_text(text.to_owned())
        .map_err(|e| format!("couldn't write to clipboard: {e}"))
}
