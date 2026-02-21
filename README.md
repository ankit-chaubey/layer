# layer

A modular, auto-generated Rust library for the Telegram [MTProto](https://core.telegram.org/mtproto) protocol.

```
layer/
├── layer-tl-parser/   # Parses .tl schema files → AST
├── layer-tl-gen/      # Code generator: AST → Rust (build-time)
├── layer-tl-types/    # Auto-generated types, functions & enums
├── layer-mtproto/     # Session state, message framing, transport traits
└── layer/             # Convenience facade re-exporting everything
```

## Highlights

| Feature | Detail |
|---------|--------|
| **Auto-generated** | All Telegram types come from `tl/api.tl` — replace the file, rebuild |
| **Updatable in one step** | Bump the `.tl` schema → `cargo build` regenerates everything |
| **Raw API ready** | Every function is a plain `struct` that implements `RemoteCall` |
| **Transport-agnostic** | Implement the `Transport` trait over TCP, WebSocket, etc. |
| **Zero unsafe** | `#![deny(unsafe_code)]` across all crates |
| **Feature flags** | Pay only for what you use |

---

## Quick start

```rust
use layer::{Session, tl::{functions, Serializable}};

let req = functions::help::GetConfig {};

let mut session = Session::new();
let msg = session.pack(&req);
let wire = msg.to_plaintext_bytes();
```

---

## Updating to a new API layer

1. Replace `layer-tl-types/tl/api.tl` with the new schema.
2. Run `cargo build` — done.

The `LAYER` constant updates automatically.

---

## Feature flags

| Flag | Default | Effect |
|------|---------|--------|
| `tl-api` | ✅ | High-level API schema |
| `tl-mtproto` | ❌ | Low-level MTProto schema |
| `impl-debug` | ✅ | Derive `Debug` |
| `impl-from-type` | ✅ | `From<types::T> for enums::E` |
| `impl-from-enum` | ✅ | `TryFrom<enums::E> for types::T` |
| `deserializable-functions` | ❌ | Server-side function deserialization |
| `name-for-id` | ❌ | `name_for_id(u32)` debug helper |
| `impl-serde` | ❌ | serde support |

---

## Architecture

```
.tl file
   │
   ▼
layer-tl-parser ──► AST
   │
   ▼
layer-tl-gen ──────► Rust source (structs/enums + impls)
   │  (build.rs)
   ▼
layer-tl-types ────► compiled types + Serializable + Deserializable
   │
   ▼
layer-mtproto ─────► Session + Message + Transport trait
   │
   ▼
layer (facade) ────► single ergonomic import
```

## License

MIT OR Apache-2.0
