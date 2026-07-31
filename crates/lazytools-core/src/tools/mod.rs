pub mod convert;
pub mod crypto;

use crate::registry::Tool;

/// Explicit listing — deliberately not using `inventory` or a self-registering macro.
/// Adding a tool means adding exactly one line here.
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
