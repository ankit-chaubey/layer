<div align="center">

# 📦 layer-tl-types

**Auto-generated Rust types for all Telegram API Layer 222 constructors, functions and enums.**

[![Crates.io](https://img.shields.io/crates/v/layer-tl-types?color=fc8d62)](https://crates.io/crates/layer-tl-types)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-2024_edition-f74c00)](https://www.rust-lang.org/)
[![TL Layer](https://img.shields.io/badge/TL%20Layer-222-8b5cf6)](https://core.telegram.org/schema)

*2,295 TL constructors. Every Telegram type, function and enum — as idiomatic Rust.*

</div>

---

## 📦 Installation

```toml
[dependencies]
layer-tl-types = "0.1.1"

# With MTProto low-level types too:
layer-tl-types = { version = "0.1.1", features = ["tl-mtproto"] }
```

---

## ✨ What It Does

`layer-tl-types` is the type system for the entire layer stack. It takes Telegram's `.tl` schema file (the source of truth for every type and function in the Telegram API) and, at **build time**, generates idiomatic Rust structs, enums, and trait implementations for all of them.

The result is a fully type-safe, zero-surprise Rust representation of 2,295 Telegram API definitions, with `Serialize` / `Deserialize` support baked in.

---

## 📐 Generated Structure

```rust
// Every TL constructor becomes a Rust struct
// e.g.  message#9cb490e9 id:int from_id:Peer ... = Message;
pub mod types {
    pub struct Message {
        pub id: i32,
        pub from_id: Option<enums::Peer>,
        pub peer_id: enums::Peer,
        pub message: String,
        // ... all fields
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
            pub peer: enums::InputPeer,
            pub message: String,
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
| `tl-api` | ✅ | Telegram API schema (api.tl) — all high-level types |
| `tl-mtproto` | ❌ | MTProto internal schema (mtproto.tl) — low-level types |
| `impl-debug` | ✅ | `#[derive(Debug)]` on all generated types |
| `impl-from-type` | ✅ | `From<types::T> for enums::E` conversions |
| `impl-from-enum` | ✅ | `TryFrom<enums::E> for types::T` conversions |
| `name-for-id` | ❌ | `name_for_id(u32) -> Option<&'static str>` lookup |
| `impl-serde` | ❌ | `serde::Serialize` / `Deserialize` on all types |

---

## 💡 Usage Examples

### Serializing a TL request

```rust
use layer_tl_types::{functions, Serializable};

let req = functions::messages::SendMessage {
    peer:     enums::InputPeer::PeerSelf,
    message:  "Hello!".to_string(),
    random_id: 12345,
    // ...
};

// Serialize to TL wire bytes
let bytes = req.serialize();
```

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

### Using RemoteCall

```rust
use layer_tl_types::{RemoteCall, functions::updates::GetState};

// The return type is encoded in the trait — no guessing
let req = GetState {};
// req implements RemoteCall<Return = enums::updates::State>
```

### Type conversions

```rust
use layer_tl_types::{types, enums};

// types → enums (From)
let peer: enums::Peer = types::PeerUser { user_id: 123 }.into();

// enums → types (TryFrom)
if let Ok(user) = types::PeerUser::try_from(peer) {
    println!("user_id: {}", user.user_id);
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

The `build.rs` invokes `layer-tl-gen` at compile time, so all types stay in sync with the schema automatically.

---

## 🔗 Part of the layer stack

```
layer-client
└── layer-mtproto
    └── layer-tl-types    ← you are here
        └── layer-tl-gen  (build-time codegen)
        └── layer-tl-parser (schema parser)
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
