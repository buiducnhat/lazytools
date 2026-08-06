[English](README.md) | [Tiếng Việt](README.vi.md)

# lazytools

Bộ tiện ích chạy trong terminal — offline, bàn phím-first. Giống
[it-tools](https://github.com/CorentinTh/it-tools) nhưng ở dạng TUI, và đồng thời
dùng được thẳng trong shell pipeline.

```console
$ echo -n "hello world" | lazytools hash --algo md5
5eb63bbbe01eeed093cb22bb8f5acdc3

$ echo -n "userIdFromDB" | lazytools case --style kebab
user-id-from-db

$ lazytools uuid
2bc10bd9-f274-45ac-ba91-2e875e385330

$ lazytools timestamp 1700000000 --json | jq -r .relative
2 years ago

$ lazytools ip 10.0.0.0/12 --json | jq -r .usable
1048574

$ lazytools data-format --from json --to yaml config.json > config.yaml
```

Gõ `lazytools` không tham số để mở giao diện:

```
┌ Tools ───────────────┐┌ Hash Text ─────────────────────────────────────────────────────┐
│Crypto                ││┌ Input ───────────────────────────────────────────────────────┐│
│  Hash Text           │││hello world                                                   ││
│  HMAC                │││                                                              ││
│  Bcrypt              │││                                                              ││
│  TOTP Code           │││                                                              ││
│Convert               │││                                                              ││
│  Base64              │││                                                              ││
│  Base32              ││└──────────────────────────────────────────────────────────────┘│
│  URL Encode          ││┌ Algorithm ───────────────────────────────────────────────────┐│
│  Hex                 │││‹ md5 ›                                                       ││
│  JSON Format         ││└──────────────────────────────────────────────────────────────┘│
│  Data Format         ││┌ Digest ──────────────────────────────────────────────────────┐│
│  Number Base         │││5eb63bbbe01eeed093cb22bb8f5acdc3                              ││
│  Unicode Escape      ││└──────────────────────────────────────────────────────────────┘│
│  Color Converter     ││                                                                │
│  HTML Entities       ││                                                                │
│  Byte Size           ││                                                                │
│  Duration            ││                                                                │
│Generate              ││                                                                │
│  Password            ││                                                                │
│  UUID                ││                                                                │
│  ULID                ││                                                                │
│  Random Token        ││                                                                │
└──────────────────────┘└────────────────────────────────────────────────────────────────┘
[Tab] next field [Esc] tools [^P] palette [^O] open file [^S] save file [y] copy [^T] them
```

> Lưu ý: giao diện TUI hiện chỉ hiển thị tiếng Anh (xem phần "Phím tắt" bên dưới
> để tra nghĩa từng nhãn).

## Cài đặt

**Homebrew** (macOS, Linux)

```bash
brew install buiducnhat/tap/lazytools
```

**Trình cài đặt shell** (macOS, Linux)

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/buiducnhat/lazytools/releases/latest/download/lazytools-installer.sh | sh
```

**PowerShell** (Windows)

```powershell
powershell -c "irm https://github.com/buiducnhat/lazytools/releases/latest/download/lazytools-installer.ps1 | iex"
```

**Cargo** — cần Rust 1.97 trở lên (edition 2024)

```bash
cargo install lazytools
```

Hoặc tải binary dựng sẵn từ [trang releases](https://github.com/buiducnhat/lazytools/releases).

### Build từ mã nguồn

```bash
git clone https://github.com/buiducnhat/lazytools
cd lazytools
cargo install --path crates/lazytools
```

Hoặc chạy tại chỗ: `cargo run -p lazytools`.

## Danh mục tool

36 tool trên năm category — đúng cách nhóm mà sidebar của TUI đang dùng.

**Crypto**

| Lệnh | Mô tả |
|---|---|
| `hash` | Băm văn bản bằng MD5 / SHA-1 / SHA-256 / SHA-512 |
| `hmac` | HMAC với khóa bí mật (SHA-1 / SHA-256 / SHA-512) |
| `bcrypt` | Băm mật khẩu, hoặc kiểm tra hash có khớp không |
| `totp` | Sinh mật khẩu dùng một lần theo thời gian từ secret base32 |

**Convert**

| Lệnh | Mô tả |
|---|---|
| `base64` | Văn bản ⇄ Base64, có tùy chọn bảng chữ cái URL-safe |
| `base32` | Văn bản ⇄ Base32 (RFC 4648) |
| `url` | Percent-encode / decode chuỗi URL |
| `hex` | Văn bản ⇄ hex |
| `json-format` | Format hoặc minify JSON, giữ nguyên thứ tự khóa |
| `data-format` | Chuyển đổi giữa JSON, YAML, TOML và CSV |
| `number-base` | Chuyển số giữa nhị phân, bát phân, thập phân và hex |
| `unicode` | Escape văn bản thành chuỗi Unicode, hoặc giải mã ngược lại |
| `color` | Chuyển màu giữa hex, RGB, HSL, HSV và CMYK |
| `html-entity` | Escape văn bản cho HTML, hoặc giải mã entity ngược lại |
| `byte-size` | Số byte ở dạng thô, đơn vị nhị phân (KiB) và thập phân (kB) |
| `duration` | Khoảng thời gian dạng giây, đồng hồ, chữ dễ đọc và ISO 8601 |

**Generate**

| Lệnh | Mô tả |
|---|---|
| `password` | Sinh mật khẩu ngẫu nhiên |
| `uuid` | Sinh UUID ngẫu nhiên (v4 hoặc v7 sắp theo thời gian) |
| `ulid` | Sinh ULID sắp xếp được theo thứ tự từ điển |
| `token` | Sinh token ngẫu nhiên N byte |
| `lorem` | Sinh văn bản giả lorem ipsum |

**Text**

| Lệnh | Mô tả |
|---|---|
| `case` | Chuyển văn bản giữa camel, snake, kebab và các kiểu khác |
| `stats` | Đếm ký tự, từ, dòng và byte trong văn bản |
| `lines` | Sắp xếp, khử trùng lặp, trim và đánh số các dòng văn bản |
| `diff` | So sánh hai khối văn bản theo dòng, theo từ hoặc theo ký tự |
| `regex` | Kiểm thử biểu thức chính quy trên văn bản và xem mọi kết quả khớp |
| `slug` | Biến một tiêu đề thành slug an toàn cho URL |
| `escape` | Escape / unescape văn bản cho chuỗi JSON, regex hoặc shell |

**Web**

| Lệnh | Mô tả |
|---|---|
| `jwt-decode` | Giải mã JWT và tuỳ chọn xác minh chữ ký HMAC |
| `jwt-encode` | Ký payload JSON thành JWT có chữ ký HMAC |
| `timestamp` | Chuyển đổi giữa Unix timestamp và ngày giờ đọc được |
| `cron` | Giải thích biểu thức cron và liệt kê các lần chạy kế tiếp |
| `url-parse` | Tách URL thành các thành phần |
| `json-diff` | So sánh hai tài liệu JSON theo cấu trúc |
| `ip` | Tách khối CIDR thành network, mask, dải địa chỉ và số host |
| `http-status` | Tra ý nghĩa của một mã trạng thái HTTP |

`lazytools <lệnh> --help` cho biết đầy đủ tùy chọn — phần trợ giúp đó **sinh
thẳng từ khai báo của tool**, nên không bao giờ lệch với hành vi thật.

## Dùng trong pipeline

- Một output → in **raw**, không nhãn, không trang trí.
- Nhiều output → mỗi dòng một cặp `key=value`.
- `--json` → in toàn bộ output dạng JSON.
- Input đọc từ stdin khi thiếu đối số vị trí hoặc khi truyền `-`.

## Phím tắt

Nhãn phím trong TUI hiển thị bằng tiếng Anh; bảng dưới đây dịch nghĩa từng nhãn:

| Phím | Nhãn trong TUI | Việc |
|---|---|---|
| `Tab` | `next field` | Chuyển vùng / sang field kế |
| `Esc` | `tools` | Nhảy thẳng về danh sách tool, từ bất kỳ trường nào |
| `j` `k` / `↑` `↓` | `select` | Di chuyển trong sidebar |
| `Ctrl+P` | `palette` | Palette tìm tool (khớp mờ trên tên, từ khóa, mô tả) |
| `Ctrl+T` | `theme` | Bảng chọn theme — xem trước khi di chuyển, `Enter` giữ lại, `Esc` trả về theme cũ |
| `y` | `copy` | Copy output đang chọn — có fallback OSC 52 nên chạy được qua SSH |
| `Ctrl+O` / `Ctrl+S` | `open file` / `save file` | Mở file vào input / lưu output ra file (`Ctrl+O` bị ẩn với tool không có input) |
| `Ctrl+R` | `run` | Chạy / sinh lại, từ bất kỳ trường nào. `Enter` cũng vậy, trừ trong trường nhiều dòng — ở đó nó xuống dòng |
| `?` | `help` | Trợ giúp |
| `Ctrl+Q` | `quit` | Thoát — dùng `Ctrl` để một phím `q` lỡ tay trong form không kết thúc phiên làm việc |

Đổi phím bằng `~/.config/lazytools/keys.toml`:

```toml
palette = "ctrl+k"
theme = "ctrl+g"
help = "?"
```

## Cấu hình

Mọi thứ ngoài phím tắt nằm trong `~/.config/lazytools/config.toml` (có tôn trọng
`$XDG_CONFIG_HOME`):

```toml
[session]
# "off" | "options" (mặc định) | "all"
restore = "options"

[theme]
# Một trong các theme có sẵn, hoặc bỏ trống để dùng màu của chính terminal.
name = "dracula"
# Các màu chỉnh riêng, áp lên trên theme đang dùng.
border_focus = "magenta"
title = "#ff8800"
text_dim = "244"
```

**`[session]`** — lazytools mở lại đúng tool bạn đang dùng, với các option y như
lúc thoát. Các trường input **không** được lưu mặc định: trong catalog này input
thường là JWT hay API token, và một tiện ích không nên tự ý giữ chúng trên đĩa.
Đặt `restore = "all"` nếu muốn lưu cả input, hoặc `"off"` để không lưu gì —
`"off"` còn xóa luôn file session do thiết lập trước đó để lại.

**Trường mật khẩu / khóa không bao giờ được ghi ra đĩa, ở bất kỳ chế độ nào.**
Session nằm ở `~/.local/state/lazytools/session.toml`; xóa nó chỉ mất thông tin
tool nào đang mở.

**`[theme]`** — chọn một theme có sẵn bằng `name`, cộng thêm các màu chỉnh
riêng. Có mười một theme: `terminal` (mặc định), `dracula`, `nord`,
`gruvbox-dark`, `solarized-dark`, `catppuccin-mocha`, `tokyo-night`,
`one-dark`, `monokai`, `solarized-light` và `github-light`.

`Ctrl+T` mở bảng chọn theme, và nó **xem trước ngay khi bạn di chuyển** — toàn
bộ giao diện phía sau popup đổi màu theo, nên bạn chọn bằng cách nhìn công cụ
thật của mình chứ không nhìn ô màu mẫu. `Enter` giữ theme đó và nhớ cho lần
sau; `Esc` trả lại theme lúc bạn mới mở bảng chọn.

Lựa chọn đó được ghi vào `~/.local/state/lazytools/theme.toml`, không bao giờ
ghi vào `config.toml` — lazytools không sửa file do bạn tự viết. Nếu bạn tự sửa
`[theme] name` thì bản sửa đó luôn thắng lựa chọn cũ trong app, và xóa file
state cũng trả quyền quyết định về cho config.

Chín ô màu là `background`, `border`, `border_focus`, `text`, `text_dim`,
`error`, `selection_fg`, `selection_bg` và `title` — mỗi giá trị là tên màu,
`#rrggbb`, hoặc chỉ số bảng màu `0`–`255`. Chúng áp lên trên theme đã chọn, nên
`name = "nord"` kèm `error = "magenta"` là Nord đổi đúng một màu. Theme
`terminal` dựng hoàn toàn từ tên màu và không tự tô nền, nhờ vậy nó bám theo
terminal của bạn dù sáng hay tối.

Config hỏng **không chặn app khởi động** — lazytools vẫn mở với giá trị mặc định
và báo rõ mục nào bị bỏ qua, để bạn vào sửa được.

### Copy qua SSH

`y` ghi vào clipboard hệ thống, và fallback sang escape sequence OSC 52 khi
không có clipboard — nhờ vậy copy qua SSH vẫn chạy. Trong phiên SSH thì thứ tự
đảo lại: thử clipboard của terminal trước, vì đó mới là máy bạn dán được. Trong
tmux, sequence được bọc lại cho `allow-passthrough`; GNU screen thì không
chuyển tiếp được, và lazytools báo thẳng thay vì giả vờ đã copy xong.

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
                .describe("Reverse a piece of text")
                .keywords(&["reverse", "flip"])
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

> Lưu ý: các chuỗi trong `ToolSpec` (`.describe(...)`, `.keywords(...)`, các
> `.label(...)`) là văn bản người dùng nhìn thấy trong TUI/CLI, nên viết bằng
> tiếng Anh để nhất quán với phần còn lại của giao diện — kể cả khi bạn đang
> đọc bản README tiếng Việt này.

Rồi thêm đúng một dòng vào `tools/mod.rs`:

```rust
Box::new(text::reverse::ReverseTool::default()),
```

Xong. Tool xuất hiện **đồng thời** ở sidebar TUI, ở palette, và ở
`lazytools --help` — form nhập được dựng tự động từ `ToolSpec`, subcommand CLI
cũng vậy.

### Vì sao lại được như thế

`lazytools-core` không phụ thuộc ratatui/crossterm/clap. Mỗi tool chỉ khai một
`ToolSpec` (mô tả field) và một hàm `Inputs → Outputs` duy nhất. Cả hai frontend
đều **đọc** spec đó chứ không hard-code gì:

- `ToolFormComponent` dựng widget theo từng `FieldKind`.
- Tầng CLI dựng subcommand + flag từ cùng spec ấy.

Hệ quả: chỉ có một nguồn sự thật, và chi phí thêm tool thứ 40 cũng ngang tool
thứ 4. Điều đó **đo được**, và đã được đo: bản v0.2.0 đưa catalog từ 8 lên 22
tool, và qua cả ba đợt thêm tool thì `git diff crates/lazytools/src/` đều trả về
**rỗng**. Có test bất biến (`crates/lazytools-core/tests/spec_invariants.rs`)
duyệt toàn bộ registry để giữ tính chất này khỏi trôi.

`RunMode` được khai theo từng tool để hành vi được quyết định ngay lúc viết tool,
thay vì thành một sự cố giật UI phát hiện sau:

- `Live` chạy lại sau mỗi lần sửa, có debounce. Đây là mặc định.
- `OnDemand` đợi phím chạy — dành cho tool chạy chậm, như bcrypt cost 12
  (~250ms).
- `Generate` chạy khi mở **và** chạy lại khi bấm phím chạy, để một tool sinh
  ngẫu nhiên có thể đưa bạn mật khẩu khác mà không cần sửa gì.

`Ctrl+R` luôn chạy tool. `Enter` cũng chạy, trừ khi đang ở một trường text nhiều
dòng — ở đó phím này thuộc về trường và dùng để xuống dòng.

Các tool là hàm thuần với đúng hai ngoại lệ có chủ đích: nhóm sinh ngẫu nhiên, và
nhóm đọc đồng hồ (`timestamp`, `cron`). Cả hai được kiểm thử theo **thuộc tính** —
độ dài, bảng ký tự, thứ tự — chứ không so với giá trị cố định.

## Phát triển

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

CI chạy đúng ba lệnh trên, trên Linux + macOS + Windows.

Snapshot test của TUI dùng [`insta`](https://insta.rs); khi giao diện đổi có
chủ đích thì duyệt lại bằng `cargo insta review`.

## Tài liệu

Xem [docs/SUMMARY.md](docs/SUMMARY.md) (tiếng Anh) để biết kiến trúc, cấu trúc
codebase, chuẩn code và bối cảnh sản phẩm. Bản dịch tiếng Việt có sẵn cục bộ
trong thư mục `docs-vi/` (không được đưa vào git).

## Giấy phép

MIT
