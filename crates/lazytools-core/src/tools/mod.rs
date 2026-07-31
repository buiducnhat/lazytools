pub mod convert;
pub mod crypto;

use crate::registry::Tool;

/// Liệt kê tường minh — cố tình không dùng `inventory` hay macro tự-đăng-ký.
/// Thêm một tool = thêm đúng một dòng ở đây.
pub(crate) fn register_all() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(crypto::hash::HashTool::default()),
        Box::new(crypto::hmac::HmacTool::default()),
        Box::new(crypto::bcrypt::BcryptTool::default()),
        Box::new(convert::base64::Base64Tool::default()),
        Box::new(convert::url::UrlTool::default()),
        Box::new(convert::hex::HexTool::default()),
        Box::new(convert::json_fmt::JsonFormatTool::default()),
        Box::new(convert::data_format::DataFormatTool::default()),
    ]
}
