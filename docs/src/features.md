# Feature Flags

## layer-client

| Feature | Default | Description |
|---|---|---|
| `sqlite-session` | ❌ | SQLite-backed session storage via `rusqlite` |
| `html` | ❌ | Built-in hand-rolled HTML parser (`parse_html`, `generate_html`) |
| `html5ever` | ❌ | Spec-compliant html5ever tokenizer — overrides the built-in `html` parser |

```toml
# SQLite session only
layer-client = { version = "0.4.0", features = ["sqlite-session"] }

# HTML parsing (minimal, no extra deps)
layer-client = { version = "0.4.0", features = ["html"] }

# HTML parsing (spec-compliant, adds html5ever dep)
layer-client = { version = "0.4.0", features = ["html5ever"] }
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
| `name-for-id` | ❌ | `name_for_id(id: u32) -> Option<&'static str>` |
| `impl-serde` | ❌ | `serde::Serialize` + `serde::Deserialize` on all types |
| `deserializable-functions` | ❌ | `Deserializable` for function types (server use) |

### Example: enable serde

```toml
layer-tl-types = { version = "0.4.0", features = ["tl-api", "impl-serde"] }
```

Then:

```rust
let json = serde_json::to_string(&some_tl_type)?;
```

### Example: name_for_id (debugging)

```toml
layer-tl-types = { version = "0.4.0", features = ["tl-api", "name-for-id"] }
```

```rust
use layer_tl_types::name_for_id;

if let Some(name) = name_for_id(0x74ae4240) {
    println!("Constructor: {name}"); // → "updates"
}
```

### Example: minimal (no Debug, no conversions)

```toml
layer-tl-types = { version = "0.4.0", default-features = false, features = ["tl-api"] }
```

This reduces compile time if you don't need the convenience traits.
