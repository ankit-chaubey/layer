# layer-tl-gen

Build-time Rust code generator for Telegram's TL schema.

[![Crates.io](https://img.shields.io/crates/v/layer-tl-gen?color=fc8d62)](https://crates.io/crates/layer-tl-gen)
[![docs.rs](https://img.shields.io/badge/docs.rs-layer--tl--gen-5865F2)](https://docs.rs/layer-tl-gen)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Build-dependency only. Runs during `cargo build` via `build.rs` and produces Rust source code from a parsed TL AST.

---

## Installation

```toml
[build-dependencies]
layer-tl-gen = "0.4.7"
```

---

## Usage in `build.rs`

```rust
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

    println!("cargo:rerun-if-changed=tl/api.tl");
}
```

In `lib.rs`:

```rust
include!(concat!(env!("OUT_DIR"), "/generated_api.rs"));
```

---

## Config

| Field | Description |
|---|---|
| `impl_debug` | `#[derive(Debug)]` on all types |
| `impl_from_type` | `From<types::T> for enums::E` |
| `impl_from_enum` | `TryFrom<enums::E> for types::T` |
| `impl_serde` | `serde::Serialize` / `Deserialize` |
| `name_for_id` | `name_for_id(u32) -> Option<&'static str>` CRC32 lookup |

---

## Generated Output

For each TL constructor, generates a Rust struct with `Serializable` / `Deserializable` impls. For each abstract type, generates a Rust enum with discriminated deserialization on the 4-byte CRC32 ID. For each function, generates a struct implementing `RemoteCall` with the correct return type.

Module layout:

```
generated.rs
├ mod types      one struct per TL constructor
├ mod enums      one enum per TL abstract type
└ mod functions
  ├ mod account
  ├ mod auth
  ├ mod channels
  ├ mod messages
  └ ...
```

---

## Stack position

```
layer-tl-types  (consumes generated code)
└ layer-tl-gen  <-- here
  └ layer-tl-parser
```

---

## License

MIT or Apache-2.0, at your option. See [LICENSE-MIT](../LICENSE-MIT) and [LICENSE-APACHE](../LICENSE-APACHE).

**Ankit Chaubey** - [github.com/ankit-chaubey](https://github.com/ankit-chaubey)
