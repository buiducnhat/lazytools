pub mod crypto;

use crate::registry::Tool;

/// Liệt kê tường minh — cố tình không dùng `inventory` hay macro tự-đăng-ký.
/// Thêm một tool = thêm đúng một dòng ở đây.
pub(crate) fn register_all() -> Vec<Box<dyn Tool>> {
    vec![Box::new(crypto::hash::HashTool::default())]
}
