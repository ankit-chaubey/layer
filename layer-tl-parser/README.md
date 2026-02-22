<div align="center">

# 🔍 layer-tl-parser

**A parser for Telegram's TL (Type Language) schema files.**

[![Crates.io](https://img.shields.io/crates/v/layer-tl-parser?color=fc8d62)](https://crates.io/crates/layer-tl-parser)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-2024_edition-f74c00)](https://www.rust-lang.org/)

*Turns raw `.tl` schema text into a structured AST — the foundation of the entire type system.*

</div>

---

## 📦 Installation

```toml
[dependencies]
layer-tl-parser = "0.1.1"
```

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

// An abstract type
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
    pub name:       String,           // e.g. "message"
    pub id:         Option<u32>,      // e.g. 0x9cb490e9 (CRC)
    pub params:     Vec<Parameter>,   // field definitions
    pub ty:         Type,             // return / abstract type
    pub category:   Category,         // Type or Function
}

pub struct Parameter {
    pub name: String,     // field name
    pub ty:   ParameterType,
}

// Parameter types cover: bare, boxed, flags, conditional, generic
pub enum ParameterType {
    Flags,
    Normal { ty: Type, flag: Option<Flag> },
    Repeated { params: Vec<Parameter> },
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
            println!("Function:    {} → {}", def.name, def.ty.name);
        }
    }
}

println!("Total definitions: {}", definitions.len());
```

---

## 🔗 Part of the layer stack

```
layer-tl-types  (generated types)
└── layer-tl-gen    (code generator, uses parser)
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
