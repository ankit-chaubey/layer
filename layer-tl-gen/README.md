<div align="center">

# ⚙️ layer-tl-gen

**Build-time Rust code generator for Telegram's TL schema.**

[![Crates.io](https://img.shields.io/crates/v/layer-tl-gen?color=fc8d62)](https://crates.io/crates/layer-tl-gen)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-2024_edition-f74c00)](https://www.rust-lang.org/)

*From TL AST to idiomatic Rust structs, enums, and traits — automatically.*

</div>

---

## 📦 Installation

```toml
[build-dependencies]
layer-tl-gen = "0.1.1"
```

---

## ✨ What It Does

`layer-tl-gen` takes a parsed TL AST (from `layer-tl-parser`) and generates complete, idiomatic Rust source code — structs for constructors, enums for abstract types, and trait implementations for functions.

It runs at **build time** via `build.rs`, so the generated code is always in sync with the schema — no manual updates needed.

---

## 🏗️ What Gets Generated

For every TL definition in the schema, `layer-tl-gen` produces:

### Constructor → Rust struct
```tl
peerUser#9db1bc6d user_id:long = Peer;
```
```rust
// generated in types module
pub struct PeerUser {
    pub user_id: i64,
}
impl Serializable for PeerUser { ... }
impl Deserializable for PeerUser { ... }
```

### Abstract type → Rust enum
```tl
peerUser  = Peer;
peerChat  = Peer;
peerChannel = Peer;
```
```rust
// generated in enums module
pub enum Peer {
    User(PeerUser),
    Chat(PeerChat),
    Channel(PeerChannel),
}
impl Deserializable for Peer { ... }  // dispatches on CRC32 id
```

### Function → RemoteCall impl
```tl
messages.sendMessage#... peer:InputPeer message:string ... = Updates;
```
```rust
// generated in functions::messages module
pub struct SendMessage {
    pub peer: enums::InputPeer,
    pub message: String,
    // ...
}
impl RemoteCall for SendMessage {
    type Return = enums::Updates;
}
impl Serializable for SendMessage { ... }
```

---

## 💡 Usage in `build.rs`

```rust
// layer-tl-types/build.rs
use layer_tl_gen::generate;

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();

    generate(
        "tl/api.tl",
        &format!("{out_dir}/generated_api.rs"),
        layer_tl_gen::Config {
            impl_debug:      true,
            impl_from_type:  true,
            impl_from_enum:  true,
            impl_serde:      false,
            name_for_id:     false,
        },
    ).expect("TL code generation failed");
}
```

---

## ⚙️ Config Options

```rust
pub struct Config {
    /// Generate #[derive(Debug)] on all types
    pub impl_debug: bool,

    /// Generate From<types::T> for enums::E
    pub impl_from_type: bool,

    /// Generate TryFrom<enums::E> for types::T
    pub impl_from_enum: bool,

    /// Generate serde::Serialize / Deserialize
    pub impl_serde: bool,

    /// Generate name_for_id(u32) -> Option<&'static str>
    pub name_for_id: bool,
}
```

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
