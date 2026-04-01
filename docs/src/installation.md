# Installation

## Add to Cargo.toml

```toml
[dependencies]
layer-client = "0.4.0"
tokio        = { version = "1", features = ["full"] }
```

`layer-client` re-exports everything you need for both user clients and bots.

---

## Getting API credentials

Every Telegram API call requires an `api_id` (integer) and `api_hash` (hex string) from your registered app.

**Step-by-step:**

1. Go to **[https://my.telegram.org](https://my.telegram.org)** and log in with your phone number
2. Click **API development tools**
3. Fill in any app name, short name, platform (Desktop), and URL (can be blank)
4. Click **Create application**
5. Copy `App api_id` and `App api_hash`

> **SECURITY:** Never hardcode credentials in source code. Use environment variables or a secrets file that is in `.gitignore`.

```rust
// Good — from environment
let api_id:   i32 = std::env::var("TG_API_ID")?.parse()?;
let api_hash: String = std::env::var("TG_API_HASH")?;

// Bad — hardcoded in source
let api_id = 12345;
let api_hash = "deadbeef..."; // ← never do this in a public repo
```

---

## Bot token (bots only)

For bots, additionally get a **bot token** from [@BotFather](https://t.me/BotFather):

1. Open Telegram → search `@BotFather` → `/start`
2. Send `/newbot`
3. Choose a display name (e.g. "My Awesome Bot")
4. Choose a username ending in `bot` (e.g. `my_awesome_bot`)
5. Copy the token: `1234567890:ABCdefGHIjklMNOpqrSTUvwxYZ`

---

## Optional features

### SQLite session storage

```toml
layer-client = { version = "0.4.0", features = ["sqlite-session"] }
```

Stores session data in a SQLite database instead of a binary file. More robust for long-running servers.

### HTML entity parsing

```toml
# Built-in hand-rolled HTML parser (no extra deps)
layer-client = { version = "0.4.0", features = ["html"] }

# OR: spec-compliant html5ever tokenizer (overrides built-in)
layer-client = { version = "0.4.0", features = ["html5ever"] }
```

| Feature | Deps added | Notes |
|---|---|---|
| `html` | none | Fast, minimal, covers common Telegram HTML tags |
| `html5ever` | `html5ever` | Full spec-compliant tokenizer; use when parsing arbitrary HTML |

### Raw type system features (`layer-tl-types`)

If you use `layer-tl-types` directly for raw API access:

```toml
layer-tl-types = { version = "0.4.0", features = [
    "tl-api",          # Telegram API types (required)
    "tl-mtproto",      # Low-level MTProto types
    "impl-debug",      # Debug trait on all types (default ON)
    "impl-from-type",  # From<types::T> for enums::E (default ON)
    "impl-from-enum",  # TryFrom<enums::E> for types::T (default ON)
    "name-for-id",     # name_for_id(u32) -> Option<&'static str>
    "impl-serde",      # serde::Serialize / Deserialize
] }
```

| Feature | Default | What it enables |
|---|---|---|
| `tl-api` | ✅ | All Telegram API constructors and functions |
| `tl-mtproto` | ❌ | Low-level MTProto transport types |
| `impl-debug` | ✅ | `#[derive(Debug)]` on every generated type |
| `impl-from-type` | ✅ | `From<types::Message> for enums::Message` |
| `impl-from-enum` | ✅ | `TryFrom<enums::Message> for types::Message` |
| `name-for-id` | ❌ | Look up constructor name by ID — useful for debugging |
| `impl-serde` | ❌ | JSON serialization via serde |

---

## Verifying installation

```rust
use layer_tl_types::LAYER;

fn main() {
    println!("Using Telegram API Layer {}", LAYER);
    // → Using Telegram API Layer 224
}
```

---

## Platform notes

| Platform | Status | Notes |
|---|---|---|
| Linux x86_64 | ✅ Fully supported | |
| macOS (Apple Silicon + Intel) | ✅ Fully supported | |
| Windows | ✅ Supported | Use WSL2 for best experience |
| Android (Termux) | ✅ Works | Native ARM64 |
| iOS | ⚠️ Untested | No async runtime constraints |
