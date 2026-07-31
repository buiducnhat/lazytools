use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, RunMode, ToolSpec};
use crate::value::{Inputs, Outputs};

const MODES: &[&str] = &["hash", "verify"];

pub struct BcryptTool {
    spec: ToolSpec,
}

impl Default for BcryptTool {
    fn default() -> Self {
        Self {
            // `OnDemand` là bắt buộc: cost 12 mất ~250ms, chạy live sẽ treo UI.
            // Đây chính là lý do `RunMode` tồn tại trong spec.
            spec: ToolSpec::new("crypto.bcrypt", "Bcrypt", Category::Crypto)
                .describe("Băm mật khẩu bằng bcrypt, hoặc kiểm tra một hash có khớp không")
                .keywords(&["bcrypt", "password", "hash", "verify", "cost"])
                .mode(RunMode::OnDemand)
                .input(Field::text("text").label("Password"))
                .option(Field::select("mode", MODES).default("hash").label("Mode"))
                .option(
                    Field::number("cost", 4, 15)
                        .default(12i64)
                        .label("Cost")
                        .help("Cost càng cao càng chậm — 12 là mặc định hợp lý"),
                )
                .option(
                    Field::text("hash")
                        .label("Hash")
                        .help("Chỉ dùng ở chế độ verify"),
                )
                .output(Field::text("result").mono().label("Result")),
        }
    }
}

impl Tool for BcryptTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let password = i.text("text");

        let result = match i.choice("mode") {
            "verify" => {
                let hash = i.text("hash");
                if hash.is_empty() {
                    return Err(ToolError::invalid(
                        "hash",
                        "chế độ verify cần một hash để so sánh",
                    ));
                }
                bcrypt::verify(password, hash)
                    .map_err(|e| ToolError::invalid("hash", format!("hash không hợp lệ: {e}")))?
                    .to_string()
            }
            _ => {
                let cost = i.num("cost").clamp(4, 15) as u32;
                bcrypt::hash(password, cost)
                    .map_err(|e| ToolError::Failed(format!("bcrypt lỗi: {e}")))?
            }
        };

        Ok(Outputs::one("result", result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::RunMode;
    use crate::value::Value;

    /// Cost thấp nhất để test chạy nhanh — 4 là mức tối thiểu bcrypt cho phép.
    const FAST: i64 = 4;

    fn run(text: &str, mode: &str, cost: i64, hash: &str) -> Result<Outputs, ToolError> {
        BcryptTool::default().run(
            &Inputs::new()
                .with("text", text)
                .with("mode", mode)
                .with("cost", Value::Num(cost))
                .with("hash", hash),
        )
    }

    #[test]
    fn declares_on_demand_because_it_is_slow() {
        assert_eq!(BcryptTool::default().spec().mode, RunMode::OnDemand);
    }

    #[test]
    fn hash_then_verify_round_trip() {
        let hash = run("hunter2", "hash", FAST, "")
            .unwrap()
            .get("result")
            .unwrap()
            .as_display();
        assert!(hash.starts_with("$2"), "phải là chuỗi bcrypt: {hash}");

        let ok = run("hunter2", "verify", FAST, &hash).unwrap();
        assert_eq!(ok.get("result").unwrap().as_display(), "true");

        let bad = run("sai mật khẩu", "verify", FAST, &hash).unwrap();
        assert_eq!(bad.get("result").unwrap().as_display(), "false");
    }

    #[test]
    fn verify_without_hash_names_the_field() {
        let err = run("hunter2", "verify", FAST, "").unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidInput { field: "hash", .. }),
            "{err:?}"
        );
    }

    #[test]
    fn malformed_hash_names_the_field() {
        let err = run("hunter2", "verify", FAST, "không phải hash").unwrap_err();
        assert!(
            matches!(err, ToolError::InvalidInput { field: "hash", .. }),
            "{err:?}"
        );
    }
}
