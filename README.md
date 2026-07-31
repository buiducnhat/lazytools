# lazytools

Bộ tiện ích chạy trong terminal — offline, bàn phím-first. Giống
[it-tools](https://github.com/CorentinTh/it-tools) nhưng ở dạng TUI, và đồng thời
dùng được thẳng trong shell pipeline.

```console
$ echo -n "hello world" | lazytools hash --algo md5
5eb63bbbe01eeed093cb22bb8f5acdc3

$ echo -n "hello" | lazytools base64
aGVsbG8=

$ lazytools data-format --from json --to yaml config.json > config.yaml
```

Gõ `lazytools` không tham số để mở giao diện:

```
┌ Tools ───────────────┐┌ Hash Text ─────────────────────────────────────┐
│Crypto                ││┌ Input ───────────────────────────────────────┐│
│  Hash Text           │││hello world                                   ││
│  HMAC                ││└──────────────────────────────────────────────┘│
│  Bcrypt              ││┌ Algorithm ───────────────────────────────────┐│
│Convert               │││‹ md5 ›                                       ││
│  Base64              ││└──────────────────────────────────────────────┘│
│  URL Encode          ││┌ Digest ──────────────────────────────────────┐│
│  Hex                 │││5eb63bbbe01eeed093cb22bb8f5acdc3              ││
│  JSON Format         ││└──────────────────────────────────────────────┘│
│  Data Format         ││                                                │
└──────────────────────┘└────────────────────────────────────────────────┘
[Tab] field kế [^P] palette [y] copy [?] trợ giúp [q] thoát
```

## Cài đặt

Cần Rust 1.97 trở lên (edition 2024).

```bash
git clone https://github.com/<you>/lazy-tools
cd lazy-tools
cargo install --path crates/lazytools
```

Hoặc chạy tại chỗ: `cargo run -p lazytools`.

## Danh mục tool

| Lệnh | Mô tả |
|---|---|
| `hash` | Băm văn bản bằng MD5 / SHA-1 / SHA-256 / SHA-512 |
| `hmac` | HMAC với khóa bí mật (SHA-1 / SHA-256 / SHA-512) |
| `bcrypt` | Băm mật khẩu, hoặc kiểm tra hash có khớp không |
| `base64` | Văn bản ⇄ Base64, có tùy chọn bảng chữ cái URL-safe |
| `url` | Percent-encode / decode chuỗi URL |
| `hex` | Văn bản ⇄ hex |
| `json-format` | Format hoặc minify JSON, giữ nguyên thứ tự khóa |
| `data-format` | Chuyển đổi giữa JSON, YAML, TOML và CSV |

`lazytools <lệnh> --help` cho biết đầy đủ tùy chọn — phần trợ giúp đó **sinh
thẳng từ khai báo của tool**, nên không bao giờ lệch với hành vi thật.

## Dùng trong pipeline

- Một output → in **raw**, không nhãn, không trang trí.
- Nhiều output → mỗi dòng một cặp `key=value`.
- `--json` → in toàn bộ output dạng JSON.
- Input đọc từ stdin khi thiếu đối số vị trí hoặc khi truyền `-`.

## Phím tắt

| Phím | Việc |
|---|---|
| `Tab` | Chuyển vùng / sang field kế |
| `j` `k` / `↑` `↓` | Di chuyển trong sidebar |
| `Ctrl+P` | Palette tìm tool (khớp mờ trên tên, từ khóa, mô tả) |
| `y` | Copy output đang chọn |
| `Ctrl+O` / `Ctrl+S` | Mở file vào input / lưu output ra file |
| `?` | Trợ giúp |
| `q` | Thoát |

Đổi phím bằng `~/.config/lazytools/keys.toml`:

```toml
palette = "ctrl+k"
quit = "q"
help = "?"
```

Config hỏng **không chặn app khởi động** — lazytools vẫn mở với phím mặc định và
báo rõ mục nào bị bỏ qua, để bạn vào sửa được.

## Thêm một tool mới

Đây là phần quan trọng nhất cho việc bảo trì lâu dài, nên nó được thiết kế để
rẻ: **một file mới + một dòng trong `register_all()`**. Không đụng tới ratatui,
không đụng tới clap.

Tạo `crates/lazytools-core/src/tools/text/reverse.rs`:

```rust
use crate::{error::ToolError, registry::Tool, spec::*, value::*};

pub struct ReverseTool { spec: ToolSpec }

impl Default for ReverseTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("text.reverse", "Reverse Text", Category::Text)
                .describe("Đảo ngược văn bản")
                .keywords(&["reverse", "đảo", "flip"])
                .input(Field::text("text").multiline().label("Input"))
                .output(Field::text("result").label("Result")),
        }
    }
}

impl Tool for ReverseTool {
    fn spec(&self) -> &ToolSpec { &self.spec }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        Ok(Outputs::one("result", i.text("text").chars().rev().collect::<String>()))
    }
}
```

Rồi thêm đúng một dòng vào `tools/mod.rs`:

```rust
Box::new(text::reverse::ReverseTool::default()),
```

Xong. Tool xuất hiện **đồng thời** ở sidebar TUI, ở palette, và ở
`lazytools --help` — form nhập được dựng tự động từ `ToolSpec`, subcommand CLI
cũng vậy.

### Vì sao lại được như thế

`lazytools-core` không phụ thuộc ratatui/crossterm/clap. Mỗi tool chỉ khai một
`ToolSpec` (mô tả field) và một hàm thuần `Inputs → Outputs`. Cả hai frontend
đều **đọc** spec đó chứ không hard-code gì:

- `ToolFormComponent` dựng widget theo từng `FieldKind`.
- Tầng CLI dựng subcommand + flag từ cùng spec ấy.

Hệ quả: chỉ có một nguồn sự thật, và chi phí thêm tool thứ 40 cũng ngang tool
thứ 4. Có test bất biến (`crates/lazytools-core/tests/spec_invariants.rs`) duyệt
toàn bộ registry để giữ tính chất này khỏi trôi.

`RunMode::OnDemand` dành cho tool chạy chậm (bcrypt cost 12 mất ~250ms) — khai
trong spec để ràng buộc hiển hiện ngay lúc viết tool, thay vì thành một sự cố
giật UI phát hiện sau.

## Phát triển

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

CI chạy đúng ba lệnh trên, trên Linux + macOS + Windows.

Snapshot test của TUI dùng [`insta`](https://insta.rs); khi giao diện đổi có
chủ đích thì duyệt lại bằng `cargo insta review`.

## Giấy phép

MIT
