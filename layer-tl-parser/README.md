<div align="center">

<img src="https://raw.githubusercontent.com/ankit-chaubey/layer/main/docs/images/crate-tl-parser-banner.svg" alt="layer-tl-parser" width="100%" />

# 🔍 layer-tl-parser

**A parser for Telegram's TL (Type Language) schema files.**

[![Crates.io](https://img.shields.io/crates/v/layer-tl-parser?color=fc8d62)](https://crates.io/crates/layer-tl-parser)
[![docs.rs](https://img.shields.io/badge/docs.rs-layer--tl--parser-5865F2)](https://docs.rs/layer-tl-parser)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-2024_edition-f74c00)](https://www.rust-lang.org/)

*Turns raw `.tl` schema text into a structured AST — the foundation of the entire type system.*

</div>

---

## 📦 Installation

```toml
[dependencies]
layer-tl-parser = "0.4.6"
```

> **Note:** Most users don't depend on this crate directly. It is used internally by `layer-tl-gen` (as a `build-dependency`) which in turn is used by `layer-tl-types` to generate all Telegram API types at build time.

---

## ✨ What It Does

The Telegram API is defined in a custom schema language called **TL (Type Language)**. Every type, constructor, and function in the Telegram protocol is described in `.tl` files — `api.tl` for the high-level API, `mtproto.tl` for the low-level MTProto protocol.

`layer-tl-parser` reads these schema files and produces a structured AST that can be consumed by code generators (like `layer-tl-gen`) to produce native Rust types.

---

## 📐 TL Schema Format

A `.tl` file looks like this:

```
// A constructor (type definition)
message#9cb490e9 flags:# out:flags.1?true id:int peer_id:Peer message:string = Message;

// A function (RPC call)
messages.sendMessage#545cd15a peer:InputPeer message:string random_id:long = Updates;

// An abstract type with multiple constructors
inputPeerEmpty#7f3b18ea = InputPeer;
inputPeerSelf#7da07ec9 = InputPeer;
inputPeerUser#dde8a54c user_id:long access_hash:long = InputPeer;
```

`layer-tl-parser` parses all of this into typed Rust structures.

---

## 🏗️ AST Structure

```rust
/// A single parsed TL definition (constructor or function)
pub struct Definition {
    pub name:     String,         // e.g. "message"
    pub id:       Option<u32>,    // e.g. 0x9cb490e9 (CRC32, may be omitted)
    pub params:   Vec<Parameter>, // field definitions
    pub ty:       Type,           // return type / abstract type name
    pub category: Category,       // Type or Function
}

pub struct Parameter {
    pub name: String,
    pub ty:   ParameterType,
}

/// Parameter types cover: bare, boxed, flags field, conditional, repeated
pub enum ParameterType {
    /// A `flags:#` field — holds the bitfield for conditional parameters
    Flags,
    /// A normal or conditional parameter
    Normal {
        ty:   Type,
        flag: Option<Flag>,   // present if this field is conditional (flags.N?type)
    },
    /// A repeated block (rare, used in some MTProto types)
    Repeated {
        params: Vec<Parameter>,
    },
}

pub enum Category {
    /// This definition is a TL constructor (contributes to an abstract type)
    Type,
    /// This definition is a TL function (an RPC call)
    Function,
}
```

---

## 💡 Usage

```rust
use layer_tl_parser::{parse_tl_file, tl::Category};

let schema = std::fs::read_to_string("api.tl").unwrap();
let definitions = parse_tl_file(&schema).unwrap();

for def in &definitions {
    match def.category {
        Category::Type => {
            println!("Constructor: {} → {}", def.name, def.ty.name);
        }
        Category::Function => {
            println!("Function:    {} (returns {})", def.name, def.ty.name);
        }
    }
}

let types_count = definitions.iter().filter(|d| d.category == Category::Type).count();
let fns_count   = definitions.iter().filter(|d| d.category == Category::Function).count();
println!("{} constructors, {} functions", types_count, fns_count);
```

---

## 🔄 How the Iterator Works

The parser exposes a **streaming iterator** (`TlIterator`) over definitions, not a batch parse. This lets the code generator process one definition at a time without holding the full AST in memory.

`parse_tl_file()` collects the iterator into a `Vec<Definition>` for convenience. If you need streaming, use the iterator directly:

```rust
use layer_tl_parser::TlIterator;

let schema = std::fs::read_to_string("api.tl").unwrap();
for def in TlIterator::new(&schema) {
    // process each definition as it is parsed
}
```

---

## ⚠️ Error Handling

Parse errors are returned as `layer_tl_parser::ParseError` with the line content that failed to parse. Unknown or malformed tokens cause the iterator to stop and return an error rather than silently skipping.

---

## 🔗 Part of the layer stack

```
layer-tl-types  (generated types, user-facing)
└── layer-tl-gen    (code generator, build-time)
    └── layer-tl-parser    ← you are here
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
