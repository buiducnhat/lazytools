---
name: lazytools-cli
description: Use lazytools CLI for quick terminal operations — hashing, encoding, decoding, generating passwords/tokens/UUIDs, converting data formats, parsing URLs/JWTs/timestamps, CIDR calculations. Invoke whenever you need to compute a hash, encode/decode base64 or URL, format JSON, generate a UUID, parse a JWT, or any of the 36 tools. Works offline, no API calls needed.
---

# lazytools CLI Skill

`lazytools` is a terminal utility belt — offline, keyboard-first. It has **36 tools** across 5 categories. Use it instead of reaching for external APIs or online tools.

## Installation

If `lazytools` is not found, install it first:

```bash
# macOS/Linux - Homebrew
brew install buiducnhat/tap/lazytools

# Or shell installer
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/buiducnhat/lazytools/releases/latest/download/lazytools-installer.sh | sh

# Cargo (requires Rust)
cargo install lazytools
```

Verify: `lazytools --help`

## Quick Reference by Category

### Crypto
```bash
# Hash text (MD5, SHA-1, SHA-256, etc.)
lazytools hash --text "password" --algo sha256
echo -n "password" | lazytools hash --algo sha256

# HMAC with secret key
lazytools hmac --text "message" --key "secret123" --algo sha256
echo -n "message" | lazytools hmac --key "secret123" --algo sha256

# Bcrypt hash/check (mode: hash|verify, default: hash)
echo -n "password" | lazytools bcrypt --cost 12
echo -n "password" | lazytools bcrypt --mode verify --hash "$2b$12$..."

# TOTP (Google Authenticator compatible)
lazytools totp --secret "JBSWY3DPEHPK3PXP" --json
```

### Convert
```bash
# Base64 (direction: encode|decode, default is encode)
echo -n "hello" | lazytools base64
echo -n "aGVsbG8=" | lazytools base64 --direction decode
lazytools base64 "aGVsbG8="  # decode inline

# Base64 URL-safe variant
echo -n "hello" | lazytools base64 --url-safe

# Base32 encode/decode
echo -n "hello" | lazytools base32
lazytools base32 "JBSWY3DPEHPK3PXP" --direction decode

# URL encoding
echo -n "hello world&foo=bar" | lazytools url
lazytools url --direction decode "hello%20world"

# Hex
echo -n "hello" | lazytools hex
lazytools hex --direction decode "68656c6c6f"

# HTML entities
echo -n "<script>" | lazytools html-entity
lazytools html-entity --direction decode "&lt;script&gt;"

# Unicode escape
echo -n "héllo" | lazytools unicode
lazytools unicode --direction decode "éllo"

# JSON format/minify
echo '{"a":1}' | lazytools json-format
lazytools json-format --mode minify config.json

# Data format conversion (JSON, YAML, TOML, CSV)
lazytools data-format --from json --to yaml '{"key":"value"}'
lazytools data-format --from yaml --to json config.yaml

# Number base conversion (outputs all formats by default)
lazytools number-base 255
lazytools number-base 0xFF
lazytools number-base 0b1010 --json

# Color conversion (auto-detects input format)
lazytools color "#3b82f6"
lazytools color "rgb(59,130,246)"
lazytools color "hsl(217, 91%, 60%)"

# Duration conversion (accepts: seconds, clock, or ISO format)
lazytools duration 3661  # → shows all formats
lazytools duration "1h30m" --unit s
lazytools duration "1:30:00" --unit s

# Byte size (accepts: bare numbers or units like "1.5 GiB")
lazytools byte-size 1048576
lazytools byte-size "1.5 GiB"
```

### Generate
```bash
# Password (charset: alphanumeric, alphanumeric+symbols, letters, digits, hex)
lazytools password --length 32 --charset alphanumeric+symbols
lazytools password --length 16 --charset letters

# UUID v4 or v7 (time-ordered)
lazytools uuid
lazytools uuid --version v7
lazytools uuid --format uppercase
lazytools uuid --count 5  # generate multiple

# ULID
lazytools ulid

# Random token (N bytes → base64)
lazytools token --bytes 32

# Lorem ipsum (unit: words|sentences|paragraphs)
lazytools lorem --unit paragraphs --count 2
```

### Text
```bash
# Case conversion
lazytools case --text "helloWorld" --style kebab
echo -n "helloWorld" | lazytools case --style kebab
# Styles: camel, pascal, snake, kebab, constant, title, lower, upper (default: snake)

# Text statistics
lazytools stats --text "hello world"
echo -n "hello world" | lazytools stats

# Line operations (sort, dedup, trim, drop empties, number)
echo -e "b\na\nb" | lazytools lines --order asc --unique
echo -e "b\na\nb" | lazytools lines --unique  # just dedup
echo "a\n  b  \nc" | lazytools lines  # auto-trims by default

# Diff (line, word, char)
lazytools diff --left "hello world" --right "hellO world" --granularity word

# Regex tester (PATTERN first, then TEXT)
lazytools regex --pattern "\d+" --text "abc123def456"

# Slug generation
echo -n "Hello World!" | lazytools slug

# Escape for JSON/regex/shell
echo -n 'He said "hello"' | lazytools escape --target json
```

### Web
```bash
# JWT decode (header, payload, signature)
lazytools jwt-decode "eyJhbGciOiJIUzI1NiJ9..."
echo -n "jwt..." | lazytools jwt-decode --secret "your-secret"  # verify signature

# JWT encode
lazytools jwt-encode --secret "your-secret" '{"sub":"user123"}'

# Timestamp conversion (auto-detects s/ms/us/ns)
lazytools timestamp 1700000000
lazytools timestamp 1700000000000 --unit ms
lazytools timestamp --timezone local  # current time

# Cron expression parser (default: next 5 runs)
lazytools cron "0 9 * * MON-FRI"

# URL parser
lazytools url-parse "https://api.example.com:8080/v1/users?id=123" --json

# JSON diff (left reads stdin, right is positional)
echo '{"a":1}' | lazytools json-diff '{"a":2}'

# CIDR calculator
lazytools ip 10.0.0.0/24 --json

# HTTP status code lookup
lazytools http-status 404
lazytools http-status 500
```

## CLI Patterns

### Input Sources
```bash
# Stdin (primary input)
echo -n "text" | lazytools base64

# Positional argument
lazytools base64 "aGVsbG8="  # auto-detects decode mode

# File input
lazytools json-format config.json
lazytools json-format < config.json
```

### JSON Output
```bash
# All outputs as JSON (for scripting)
lazytools ip 192.168.0.0/24 --json

# Single value → raw value (no label)
echo -n "hello" | lazytools base64
# Output: aGVsbG8=
```

### Piping
```bash
# Chain tools
echo -n "hello" | lazytools base64 | lazytools base64 --direction decode
lazytools uuid | xargs -I{} lazytools jwt-encode --secret "key" '{"sub":"{}"}'
```

## When to Use lazytools vs Alternatives

| Task | lazytools | Alternative |
|------|-----------|-------------|
| Quick hash | `lazytools hash` | Opening a website |
| Base64 encode/decode | `lazytools base64` | `base64` command (macOS/Linux differ) |
| JWT decode | `lazytools jwt-decode` | jwt.io website |
| UUID generation | `lazytools uuid` | Online generator |
| JSON formatting | `lazytools json-format` | `jq` (but jq needs install) |
| Format conversion | `lazytools data-format` | Python one-liner |
| Password gen | `lazytools password` | `openssl rand` |
| Timestamp | `lazytools timestamp` | `date` command |

**Use lazytools when**: You need something quick, offline-capable, and consistent across platforms. The CLI output is pipe-friendly.

**Use specialized tools when**: You already have them installed and they're more powerful for your specific use case (e.g., `jq` for complex JSON transformations).

## Pro Tips

1. **No install needed for simple checks**: If `lazytools` isn't available and the task is trivial, consider if a shell builtin (`date`, `printf %s`) or built-in language feature would work first.

2. **JSON output for parsing**: Always use `--json` when you need to extract specific fields:
   ```bash
   lazytools ip 10.0.0.0/24 --json | jq -r '.network'
   ```

3. **Stdin is the primary input**: Most tools read from stdin if no positional argument is given. This makes piping natural.

4. **Error messages are helpful**: If a tool fails, read the error — it often tells you exactly what's wrong (e.g., "invalid base64", "malformed JSON").
