<div align="center">

<img src="https://raw.githubusercontent.com/ankit-chaubey/layer/main/docs/images/crate-tl-gen-banner.svg" alt="layer-tl-gen" width="100%" />

# ⚙️ layer-tl-gen

**Build-time Rust code generator for Telegram's TL schema.**

[![Crates.io](https://img.shields.io/crates/v/layer-tl-gen?color=fc8d62)](https://crates.io/crates/layer-tl-gen)
[![docs.rs](https://img.shields.io/badge/docs.rs-layer--tl--gen-5865F2)](https://docs.rs/layer-tl-gen)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-2024_edition-f74c00)](https://www.rust-lang.org/)

*From TL AST to idiomatic Rust structs, enums, and traits — automatically.*

</div>

---

## 📦 Installation

```toml
[build-dependencies]
layer-tl-gen = "0.4.5"
```

> This crate is a **build-dependency** only. It runs during `cargo build` via `build.rs` and produces Rust source code. It is not linked into your final binary.

---

## ✨ What It Does

`layer-tl-gen` takes a parsed TL AST (from `layer-tl-parser`) and generates complete, idiomatic Rust source code — structs for constructors, enums for abstract types, and trait implementations for functions.

It runs at **build time** via `build.rs`, so the generated code is always in sync with the schema — no manual updates needed. Regeneration is automatic whenever the `.tl` file changes (via `cargo:rerun-if-changed`).

---

## 🏗️ What Gets Generated

For every TL definition in the schema, `layer-tl-gen` produces three categories of output:

### 1. Constructor → Rust struct + ser/de

```tl
peerUser#9db1bc6d user_id:long = Peer;
```
```rust
// in mod types
pub struct PeerUser {
    pub user_id: i64,
}
impl Serializable   for PeerUser { /* TL wire encoding */ }
impl Deserializable for PeerUser { /* TL wire decoding */ }
```

---

### 2. Abstract type → Rust enum + discriminated deserialization

```tl
peerUser    = Peer;
peerChat    = Peer;
peerChannel = Peer;
```
```rust
// in mod enums
pub enum Peer {
    User(PeerUser),
    Chat(PeerChat),
    Channel(PeerChannel),
}
// Deserializable dispatches on the 4-byte CRC32 constructor ID
impl Deserializable for Peer { ... }
```

---

### 3. Function → struct + `RemoteCall` impl

```tl
messages.sendMessage#545cd15a peer:InputPeer message:string random_id:long = Updates;
```
```rust
// in mod functions::messages
pub struct SendMessage {
    pub peer:      enums::InputPeer,
    pub message:   String,
    pub random_id: i64,
    // ... all fields from the schema
}
impl RemoteCall for SendMessage {
    type Return = enums::Updates;   // zero-cost type-level encoding of the return type
}
impl Serializable for SendMessage { ... }
```

---

### 4. Optional derives and conversions

Depending on the `Config` flags passed to `generate()`:

```rust
// impl_debug = true
#[derive(Debug)]
pub struct PeerUser { ... }

// impl_from_type = true
impl From<types::PeerUser> for enums::Peer {
    fn from(v: types::PeerUser) -> Self { Self::User(v) }
}

// impl_from_enum = true
impl TryFrom<enums::Peer> for types::PeerUser {
    type Error = enums::Peer;
    fn try_from(v: enums::Peer) -> Result<Self, Self::Error> { ... }
}

// name_for_id = true
pub fn name_for_id(id: u32) -> Option<&'static str> { ... }
```

---

## 💡 Usage in `build.rs`

```rust
// layer-tl-types/build.rs
use layer_tl_gen::{generate, Config};

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();

    generate(
        "tl/api.tl",
        &format!("{out_dir}/generated_api.rs"),
        Config {
            impl_debug:      true,
            impl_from_type:  true,
            impl_from_enum:  true,
            impl_serde:      false,
            name_for_id:     false,
        },
    ).expect("TL code generation failed");

    // Tell Cargo to re-run if the schema changes
    println!("cargo:rerun-if-changed=tl/api.tl");
}
```

Then in your `lib.rs` or `generated.rs`:

```rust
// include the generated file
include!(concat!(env!("OUT_DIR"), "/generated_api.rs"));
```

---

## ⚙️ Config Options

```rust
pub struct Config {
    /// Generate `#[derive(Debug)]` on all types.
    pub impl_debug: bool,

    /// Generate `From<types::T> for enums::E` for all constructors.
    pub impl_from_type: bool,

    /// Generate `TryFrom<enums::E> for types::T` for all constructors.
    pub impl_from_enum: bool,

    /// Generate `serde::Serialize` / `Deserialize` on all types.
    /// Requires the `serde` crate to be available in the consuming crate.
    pub impl_serde: bool,

    /// Generate `name_for_id(u32) -> Option<&'static str>` lookup.
    /// Maps a constructor's CRC32 ID back to its TL name — useful for debugging.
    pub name_for_id: bool,
}
```

---

## 🧩 Module Layout of Generated Code

```
generated.rs
├── mod types      — one struct per TL constructor
├── mod enums      — one enum per TL abstract type
└── mod functions
    ├── mod account
    ├── mod auth
    ├── mod channels
    ├── mod contacts
    ├── mod messages
    ├── mod payments
    ├── mod phone
    ├── mod photos
    ├── mod stickers
    ├── mod stories
    ├── mod updates
    └── mod users
```

Each `functions::*` module mirrors the namespace prefix from the TL schema (`messages.sendMessage` → `functions::messages::SendMessage`).

---

## 🔗 Part of the layer stack

```
layer-tl-types  (consumes the generated code)
└── layer-tl-gen    ← you are here  (generates at build time)
    └── layer-tl-parser (parses the .tl schema)
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
