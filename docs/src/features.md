# Feature Flags

<img src="images/feature-flags.svg" alt="layer feature flags overview" width="100%" style="margin: 0.5rem 0 1.5rem 0; border-radius:6px;" />

## layer-client

| Feature | Default | Description |
|---|---|---|
| `sqlite-session` | ❌ | SQLite-backed session storage via `rusqlite` |
| `libsql-session` | ❌ | libsql / Turso session storage — local or remote (**New in v0.4.6**) |
| `html` | ❌ | Built-in hand-rolled HTML parser (`parse_html`, `generate_html`) |
| `html5ever` | ❌ | Spec-compliant html5ever tokenizer — overrides the built-in `html` parser |
| `serde` | ❌ | `serde::Serialize` / `Deserialize` on `Config` and public structs |

The following are **always available** without any feature flag:
- `InlineKeyboard`, `ReplyKeyboard`, `Button` — keyboard builders
- `InputReactions` — reaction builder
- `TypingGuard` — RAII typing indicator
- `SearchBuilder`, `GlobalSearchBuilder` — fluent search
- `PeerRef` — flexible peer argument
- `User`, `Group`, `Channel`, `Chat` — ergonomic type wrappers
- `Socks5Config` — SOCKS5 proxy config
- `BannedRightsBuilder`, `AdminRightsBuilder`, `ParticipantPermissions`
- `StringSessionBackend`, `InMemoryBackend`, `BinaryFileBackend`
- `ClientBuilder` — fluent connection builder

```toml
# SQLite session only
layer-client = { version = "0.4.6", features = ["sqlite-session"] }

# LibSQL / Turso session (new in 0.4.6)
layer-client = { version = "0.4.6", features = ["libsql-session"] }

# HTML parsing (minimal, no extra deps)
layer-client = { version = "0.4.6", features = ["html"] }

# HTML parsing (spec-compliant, adds html5ever dep)
layer-client = { version = "0.4.6", features = ["html5ever"] }

# Multiple features at once
layer-client = { version = "0.4.6", features = ["sqlite-session", "html"] }
```

---

## layer-tl-types

| Feature | Default | Description |
|---|---|---|
| `tl-api` | ✅ | Telegram API schema (constructors, functions, enums) |
| `tl-mtproto` | ❌ | Low-level MTProto transport types |
| `impl-debug` | ✅ | `#[derive(Debug)]` on all generated types |
| `impl-from-type` | ✅ | `From<types::T> for enums::E` conversions |
| `impl-from-enum` | ✅ | `TryFrom<enums::E> for types::T` conversions |
| `deserializable-functions` | ❌ | `Deserializable` for function types (server-side use) |
| `name-for-id` | ❌ | `name_for_id(id: u32) -> Option<&'static str>` |
| `impl-serde` | ❌ | `serde::Serialize` + `serde::Deserialize` on all types |

### Example: enable serde

```toml
layer-tl-types = { version = "0.4.6", features = ["tl-api", "impl-serde"] }
```

```rust
let json = serde_json::to_string(&some_tl_type)?;
```

### Example: name_for_id (debugging)

```toml
layer-tl-types = { version = "0.4.6", features = ["tl-api", "name-for-id"] }
```

```rust
use layer_tl_types::name_for_id;

if let Some(name) = name_for_id(0x74ae4240) {
    println!("Constructor: {name}"); // → "updates"
}
```

### Example: minimal (no Debug, no conversions)

```toml
layer-tl-types = { version = "0.4.6", default-features = false, features = ["tl-api"] }
```

Reduces compile time when you don't need convenience traits.

---

## String session — no feature flag needed

`StringSessionBackend` and `export_session_string()` are available in the default build — no feature flag required:

```toml
layer-client = "0.4.6"   # already includes StringSessionBackend
```

```rust
let s = client.export_session_string().await?;
let (client, _) = Client::with_string_session(&s, api_id, api_hash).await?;
```

---

## docs.rs build matrix

When building docs on docs.rs, all feature flags are enabled:

```toml
[package.metadata.docs.rs]
features = ["sqlite-session", "libsql-session", "html", "html5ever"]
rustdoc-args = ["--cfg", "docsrs"]
```
