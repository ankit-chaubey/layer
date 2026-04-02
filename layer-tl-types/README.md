<div align="center">

<img src="https://raw.githubusercontent.com/ankit-chaubey/layer/main/docs/images/crate-tl-types-banner.svg" alt="layer-tl-types" width="100%" />

# 📦 layer-tl-types

**Auto-generated Rust types for all Telegram API Layer 224 constructors, functions and enums.**

[![Crates.io](https://img.shields.io/crates/v/layer-tl-types?color=fc8d62)](https://crates.io/crates/layer-tl-types)
[![docs.rs](https://img.shields.io/badge/docs.rs-layer--tl--types-5865F2)](https://docs.rs/layer-tl-types)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-2024_edition-f74c00)](https://www.rust-lang.org/)
[![TL Layer](https://img.shields.io/badge/TL%20Layer-224-8b5cf6)](https://core.telegram.org/schema)

*2,329 TL constructors. Every Telegram type, function and enum — as idiomatic Rust.*

</div>

---

## 📦 Installation

```toml
[dependencies]
layer-tl-types = "0.4.5"

# With MTProto low-level types too (required by layer-mtproto):
layer-tl-types = { version = "0.4.5", features = ["tl-mtproto"] }
```

---

## ✨ What It Does

`layer-tl-types` is the type system for the entire layer stack. It takes Telegram's `.tl` schema file (the source of truth for every type and function in the Telegram API) and, at **build time**, generates idiomatic Rust structs, enums, and trait implementations for all of them.

The result is a fully type-safe, zero-surprise Rust representation of 2,329 Telegram API definitions, with binary TL serialization and deserialization baked in via the `Serializable` and `Deserializable` traits.

---

## 📐 Generated Structure

```rust
// Every TL constructor becomes a Rust struct
// e.g.  message#9cb490e9 id:int from_id:Peer ... = Message;
pub mod types {
    pub struct Message {
        pub id:       i32,
        pub from_id:  Option<enums::Peer>,
        pub peer_id:  enums::Peer,
        pub message:  String,
        // ... all fields, optional ones wrapped in Option<>
    }
}

// Every abstract TL type becomes a Rust enum
// e.g.  message | messageEmpty | messageService = Message
pub mod enums {
    pub enum Message {
        Message(types::Message),
        Service(types::MessageService),
        Empty(types::MessageEmpty),
    }
}

// Every TL function becomes a struct implementing RemoteCall
// e.g.  messages.sendMessage#... peer:InputPeer message:string ... = Updates
pub mod functions {
    pub mod messages {
        pub struct SendMessage {
            pub peer:      enums::InputPeer,
            pub message:   String,
            pub random_id: i64,
            // ...
        }
        impl RemoteCall for SendMessage {
            type Return = enums::Updates;
        }
    }
}
```

---

## 🔧 Feature Flags

| Feature | Default | Description |
|---|---|---|
| `tl-api` | ✅ | Telegram API schema (`api.tl`) — all high-level types |
| `tl-mtproto` | ❌ | MTProto internal schema (`mtproto.tl`) — low-level protocol types |
| `impl-debug` | ✅ | `#[derive(Debug)]` on all generated types |
| `impl-from-type` | ✅ | `From<types::T> for enums::E` conversions |
| `impl-from-enum` | ✅ | `TryFrom<enums::E> for types::T` conversions |
| `deserializable-functions` | ❌ | `Deserializable` on function types (for server-side use) |
| `name-for-id` | ❌ | `name_for_id(u32) -> Option<&'static str>` CRC32 lookup |
| `impl-serde` | ❌ | `serde::Serialize` / `Deserialize` on all types |

---

## 💡 Usage Examples

### Serializing a TL request

```rust
use layer_tl_types::{enums, functions, Serializable};

let req = functions::messages::SendMessage {
    peer:      enums::InputPeer::PeerSelf,
    message:   "Hello!".to_string(),
    random_id: 12345,
    no_webpage:   false,
    silent:       false,
    background:   false,
    clear_draft:  false,
    noforwards:   false,
    update_stickersets_order: false,
    invert_media: false,
    reply_to:     None,
    reply_markup: None,
    entities:     None,
    schedule_date: None,
    send_as:      None,
};

// Serialize to TL wire bytes (constructor ID + fields)
let bytes = req.serialize();
```

---

### Deserializing a TL response

```rust
use layer_tl_types::{Cursor, Deserializable, enums};

let mut cur = Cursor::from_slice(&response_bytes);
let msg = enums::Message::deserialize(&mut cur)?;

match msg {
    enums::Message::Message(m) => println!("text: {}", m.message),
    enums::Message::Service(s) => println!("service action"),
    enums::Message::Empty(_)   => println!("empty"),
}
```

---

### Using `RemoteCall` for type-safe RPC

```rust
use layer_tl_types::{RemoteCall, functions::updates::GetState};

// The return type is encoded as an associated type — no guessing, no casting
let req = GetState {};
// req: impl RemoteCall<Return = enums::updates::State>
```

The `RemoteCall` trait ties the request type to its response type at compile time, so `client.invoke(&req)` returns `Result<R::Return, _>` — fully typed.

---

### Type conversions

```rust
use layer_tl_types::{types, enums};

// types → enums (From, enabled by impl-from-type feature)
let peer: enums::Peer = types::PeerUser { user_id: 123 }.into();

// enums → types (TryFrom, enabled by impl-from-enum feature)
if let Ok(user) = types::PeerUser::try_from(peer) {
    println!("user_id: {}", user.user_id);
}
```

---

### Name lookup (for debugging)

```rust
// Requires the `name-for-id` feature
use layer_tl_types::name_for_id;

if let Some(name) = name_for_id(0x9cb490e9) {
    println!("{name}"); // "message"
}
```

---

## 🔄 Updating the TL Schema

To update to a newer Telegram API layer:

```bash
# 1. Replace the schema file
cp new-api.tl layer-tl-types/tl/api.tl

# 2. Rebuild — code regenerates automatically
cargo build
```

The `build.rs` invokes `layer-tl-gen` at compile time, so all types stay in sync with the schema. The `cargo:rerun-if-changed` instruction means only a schema change triggers regeneration, not every build.

---

## 🔗 Part of the layer stack

```
layer-client
└── layer-mtproto
    └── layer-tl-types    ← you are here
        └── (build) layer-tl-gen
            └── (build) layer-tl-parser
```

---

## 📄 License

Licensed under either of, at your option:

- **MIT License** — see [LICENSE-MIT](../LICENSE-MIT)
- **Apache License, Version 2.0** — see [LICENSE-APACHE](../LICENSE-APACHE)

---

## 👤 Author

**Ankit Chaubey**  
[github.com/ankit-chaubey](https://github.com/ankit-chaubey) · [ankitchaubey.in](https://ankitchaubey.in) · [ankitchaubey.dev@gmail.com](mailto:ankitchaubey.dev@gmail.com)

📦 [github.com/ankit-chaubey/layer](https://github.com/ankit-chaubey/layer)
