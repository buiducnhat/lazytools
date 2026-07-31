//! Copy ra clipboard hệ thống.
//!
//! Thất bại (thường gặp khi chạy qua SSH: không có clipboard server) phải được
//! **báo rõ**, không panic và không im lặng. Fallback OSC52 đã chốt hoãn v0.2.

/// Trả về `Err` kèm lý do đọc được cho người dùng.
pub fn copy(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|e| format!("không mở được clipboard: {e}\nQua SSH thì thường là không có."))?;
    clipboard
        .set_text(text.to_owned())
        .map_err(|e| format!("không ghi được vào clipboard: {e}"))
}
