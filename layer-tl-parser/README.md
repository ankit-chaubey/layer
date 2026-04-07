# layer-tl-parser

Parser for Telegram's TL (Type Language) schema files.

[![Crates.io](https://img.shields.io/crates/v/layer-tl-parser?color=fc8d62)](https://crates.io/crates/layer-tl-parser)
[![docs.rs](https://img.shields.io/badge/docs.rs-layer--tl--parser-5865F2)](https://docs.rs/layer-tl-parser)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Reads `.tl` schema files and produces a structured AST. Used internally by `layer-tl-gen` as a build-dependency - most users don't depend on this crate directly.

---

## Installation

```toml
[dependencies]
layer-tl-parser = "0.4.7"
```

---

## AST Types

```rust
pub struct Definition {
    pub name:     String,
    pub id:       Option<u32>,    // CRC32, may be omitted
    pub params:   Vec<Parameter>,
    pub ty:       Type,
    pub category: Category,       // Type or Function
}

pub enum ParameterType {
    Flags,
    Normal { ty: Type, flag: Option<Flag> },
    Repeated { params: Vec<Parameter> },
}

pub enum Category { Type, Function }
```

---

## Usage

```rust
use layer_tl_parser::{parse_tl_file, TlIterator, tl::Category};

// Collect all definitions
let schema = std::fs::read_to_string("api.tl").unwrap();
let definitions = parse_tl_file(&schema).unwrap();

// Streaming iterator (lower memory)
for def in TlIterator::new(&schema) {
    match def.category {
        Category::Type     => { /* constructor */ }
        Category::Function => { /* RPC function */ }
    }
}
```

Parse errors return `ParseError` with the failing line. Malformed tokens stop the iterator rather than silently skipping.

---

## Stack position

```
layer-tl-types
└ layer-tl-gen
  └ layer-tl-parser  <-- here
```

---

## License

MIT or Apache-2.0, at your option. See [LICENSE-MIT](../LICENSE-MIT) and [LICENSE-APACHE](../LICENSE-APACHE).

**Ankit Chaubey** - [github.com/ankit-chaubey](https://github.com/ankit-chaubey)
