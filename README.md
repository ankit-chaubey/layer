# layer

A modular, auto-generated Rust library for the Telegram [MTProto](https://core.telegram.org/mtproto) protocol.

```
layer/
├── layer-tl-parser/   # Parses .tl schema files → AST
├── layer-tl-gen/      # Code generator: AST → Rust (runs at build time)
├── layer-tl-types/    # Auto-generated types, functions & enums
├── layer-mtproto/     # Session state, message framing, transport traits
└── layer/             # Convenience facade re-exporting everything
```

## Quick start

Add the facade crate to your project:

```toml
# Cargo.toml
[dependencies]
layer = { path = "path/to/layer/layer" }
```

Then use it:

```rust
use layer::{Session, tl::{functions, Serializable}};

// Build a request
let req = functions::help::GetConfig {};

// Pack into an MTProto message
let mut session = Session::new();
let msg = session.pack(&req);
let wire = msg.to_plaintext_bytes();

// Send `wire` over your TCP/WebSocket transport
```

## Architecture

```
api.tl / mtproto.tl
        │
        ▼
layer-tl-parser  ─── parses .tl text → AST (Definition structs)
        │
        ▼
layer-tl-gen     ─── AST → Rust source (runs inside build.rs)
        │
        ▼
layer-tl-types   ─── compiled types, functions, enums
                     + Serializable / Deserializable traits
        │
        ▼
layer-mtproto    ─── Session + Message + Transport trait
        │
        ▼
layer            ─── single ergonomic import (facade)
```

## Updating to a new API layer

1. Replace `layer-tl-types/tl/api.tl` with the new schema.
2. Run `cargo build` — the build script regenerates everything.

The `LAYER` constant updates automatically from the `// LAYER N` comment in the schema file.

## Running tests

```bash
cargo test --workspace
```

## Feature flags

| Flag | Default | Effect |
|------|---------|--------|
| `tl-api` | ✅ | High-level API schema |
| `tl-mtproto` | ❌ | Low-level MTProto schema |
| `impl-debug` | ✅ | Derive `Debug` for all types |
| `impl-from-type` | ✅ | `From<types::T> for enums::E` |
| `impl-from-enum` | ✅ | `TryFrom<enums::E> for types::T` |
| `deserializable-functions` | ❌ | Server-side function deserialization |
| `name-for-id` | ❌ | `name_for_id(u32)` debug helper |
| `impl-serde` | ❌ | serde support |

Enable features in your `Cargo.toml`:

```toml
layer = { path = "...", features = ["impl-serde", "name-for-id"] }
```

## License

MIT OR Apache-2.0
