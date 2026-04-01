# Feature Flags

## layer-client

| Feature | Default | Description |
|---|---|---|
| `sqlite-session` | ❌ | SQLite-backed session storage via `rusqlite` |

```toml
layer-client = { version = "0.2.2", features = ["sqlite-session"] }
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
layer-tl-types = { version = "0.2.2", features = ["tl-api", "impl-serde"] }
```

Then:

```rust
let json = serde_json::to_string(&some_tl_type)?;
```

### Example: name_for_id (debugging)

```toml
layer-tl-types = { version = "0.2.2", features = ["tl-api", "name-for-id"] }
```

```rust
use layer_tl_types::name_for_id;

if let Some(name) = name_for_id(0x74ae4240) {
    println!("Constructor: {name}"); // → "updates"
}
```

### Example: minimal (no Debug, no conversions)

```toml
layer-tl-types = { version = "0.2.2", default-features = false, features = ["tl-api"] }
```

This reduces compile time if you don't need the convenience traits.
