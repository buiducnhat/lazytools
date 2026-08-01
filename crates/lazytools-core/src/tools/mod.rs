pub mod convert;
pub mod crypto;
pub mod generate;
pub mod text;
pub mod web;

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
        Box::new(convert::number_base::NumberBaseTool::default()),
        Box::new(convert::unicode::UnicodeTool::default()),
        Box::new(text::case::CaseTool::default()),
        Box::new(text::stats::StatsTool::default()),
        Box::new(web::jwt_decode::JwtDecodeTool::default()),
        Box::new(generate::password::PasswordTool::default()),
        Box::new(generate::uuid::UuidTool::default()),
        Box::new(generate::ulid::UlidTool::default()),
        Box::new(generate::token::TokenTool::default()),
        Box::new(generate::lorem::LoremTool::default()),
        Box::new(web::timestamp::TimestampTool::default()),
        Box::new(web::cron::CronTool::default()),
        Box::new(web::url_parse::UrlParseTool::default()),
        Box::new(web::json_diff::JsonDiffTool::default()),
    ]
}
