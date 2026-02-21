# ⚡ layer

> A modular, from-scratch Rust implementation of the Telegram MTProto protocol.

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-2024_edition-f74c00)](https://www.rust-lang.org/)
[![TL Layer](https://img.shields.io/badge/TL%20Layer-222-8b5cf6)](https://core.telegram.org/schema)
[![Status: Experimental](https://img.shields.io/badge/status-experimental-red)](https://github.com/ankit-chaubey/layer)

---

> ⚠️ **Use at your own risk!** 😁
>
> This is an experimental, educational project built from scratch to understand Telegram's
> MTProto protocol at the lowest level. It is **not** production-ready.
> For serious projects, use [grammers](https://codeberg.org/Lonami/grammers).

---

## ✨ About

**layer** is a hand-written, bottom-up Rust implementation of the
[Telegram MTProto](https://core.telegram.org/mtproto) protocol. Every component —
from the TL schema parser, to the AES-IGE cipher, to the Diffie-Hellman key exchange —
is written and owned by this project.

Built purely for **learning and experimentation**: to understand what happens inside a
Telegram client at the raw byte level, all the way from TCP to a high-level API call.

---

## 💡 Inspiration & Credits

This project is **heavily inspired by** and **based on the architecture of**
[**grammers**](https://codeberg.org/Lonami/grammers) by
[**Lonami**](https://codeberg.org/Lonami).

> 🙏 A huge **Thank You** to [Lonami](https://codeberg.org/Lonami) for building grammers —
> an incredibly well-structured, readable library that made MTProto's internals approachable.
> Without grammers, this project simply would not exist. Thank you for the awesome library! 🎉

The flow, naming conventions, SRP 2FA math, DC migration logic, session persistence,
and overall architecture mirror grammers closely — intentionally, as a learning exercise.

**Written by:** [Ankit Chaubey](https://github.com/ankit-chaubey)  
**Inspired by:** [grammers](https://codeberg.org/Lonami/grammers) by [Lonami](https://codeberg.org/Lonami)

---

## 🏗️ Crate Structure

```
layer/
├── layer-tl-parser/   ── Parses .tl schema text → AST
├── layer-tl-gen/      ── AST → Rust source code (runs at build time)
├── layer-tl-types/    ── Auto-generated types, functions & enums (Layer 222)
├── layer-crypto/      ── AES-IGE, RSA, SHA, auth key derivation
├── layer-mtproto/     ── MTProto session, DH exchange, message framing, transport
├── layer-client/      ── High-level Client: auth, 2FA, send messages
├── layer-connect/     ── Demo binary: raw DH + getConfig
├── layer-app/         ── Binary: interactive login + send a message
└── layer/             ── Convenience facade re-exporting everything
```

The code generation pipeline runs automatically at build time:

```
api.tl / mtproto.tl
      │
      ▼
layer-tl-parser  ── .tl text → Definition AST
      │
      ▼
layer-tl-gen     ── AST → Rust source (inside build.rs)
      │
      ▼
layer-tl-types   ── compiled structs, enums, RemoteCall impls
      │
      ▼
layer-mtproto    ── Session + EncryptedSession + Transport
      │
      ▼
layer-client     ── Client::connect / sign_in / send_message
```

---

## 🚀 Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
layer-client = { git = "https://github.com/ankit-chaubey/layer" }
```

Basic usage:

```rust
use layer_client::{Client, SignInError};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connects, runs DH key exchange, calls initConnection(GetConfig)
    let mut client = Client::load_or_connect("session.bin", API_ID, API_HASH)?;

    if !client.is_authorized()? {
        let token = client.request_login_code("+1234567890")?;
        let code  = "12345"; // the code Telegram sent you

        match client.sign_in(&token, code) {
            Ok(name) => println!("Welcome, {name}!"),

            // 2FA cloud password
            Err(SignInError::PasswordRequired(pw_token)) => {
                let hint = pw_token.hint().unwrap_or("no hint");
                println!("2FA hint: {hint}");
                client.check_password(pw_token, "my_2fa_password")?;
            }

            Err(e) => return Err(e.into()),
        }

        client.save_session("session.bin")?;
    }

    client.send_message("me", "Hello from layer! 😁")?;
    Ok(())
}
```

Or just run the included app — fill in your credentials at the top of `layer-app/src/main.rs`:

```bash
cargo run -p layer-app
```

---

## ✅ What's Implemented

### 🔐 Cryptography (`layer-crypto`)

- [x] AES-IGE encryption / decryption (MTProto 2.0)
- [x] RSA encryption with Telegram's public keys
- [x] SHA-1 and SHA-256 hashing utilities
- [x] Auth key derivation from nonce material
- [x] PQ factorization (Pollard's rho algorithm)
- [x] Diffie-Hellman shared secret computation

### 📡 MTProto Layer (`layer-mtproto`)

- [x] Full 3-step Diffie-Hellman key exchange handshake
- [x] MTProto 2.0 encrypted sessions (AES-IGE + auth key)
- [x] Proper message framing (salt, session_id, msg_id, seq_no)
- [x] Abridged TCP transport
- [x] Server salt tracking and correction
- [x] `msg_container` (multi-message) unpacking
- [x] gzip-packed response decompression
- [x] Pong, bad_server_salt, new_session_created handling

### 📦 TL Type System

- [x] Full `.tl` schema parser (`layer-tl-parser`)
- [x] Build-time Rust code generation (`layer-tl-gen`)
- [x] All Layer 222 constructors — 2,295 definitions (`layer-tl-types`)
- [x] `Serializable` / `Deserializable` traits for all types
- [x] `RemoteCall` trait for all RPC functions
- [x] `From<types::T> for enums::E` conversion impls
- [x] Optional: `Debug`, `serde`, `name_for_id(u32)`

### 👤 High-Level Client (`layer-client`)

- [x] `Client::connect()` — TCP + DH + `initConnection(GetConfig)`
- [x] `Client::load_or_connect()` — reuses saved session if available
- [x] `Client::save_session()` — persists auth key + DC table to disk
- [x] `Client::is_authorized()` — probes with `updates.getState`
- [x] `Client::request_login_code()` — sends code via SMS or Telegram app
- [x] `Client::sign_in()` — phone code login
- [x] `Client::check_password()` — full SRP 2FA (same math as grammers)
- [x] `Client::send_message()` — sends text to any peer (`"me"`, username, phone)
- [x] `Client::invoke()` — raw `RemoteCall` escape hatch for any API method
- [x] DC migration (`PHONE_MIGRATE_X`, `USER_MIGRATE_X`)
- [x] RPC error propagation (code + message string)

---

## ❌ What's NOT Implemented

- [ ] **Async / Tokio** — fully synchronous, blocking I/O
- [ ] **Update handling** — no event loop, no `iter_messages()`, no update callbacks
- [ ] **Media** — no file upload, no file download, no thumbnails
- [ ] **Dialogs / contacts** — no `get_dialogs()`, `get_entity()`
- [ ] **Automatic flood wait** — `FLOOD_WAIT_X` errors surface as `Err`, not retried
- [ ] **MTProxy / obfuscation** — no proxy support
- [ ] **Multiple accounts** — single session only
- [ ] **Channels / groups** — only basic peer resolution for `send_message`
- [ ] **Bots** — no bot-specific auth flow
- [ ] **WebSocket transport** — TCP only

---

## 🆚 layer vs grammers

| Feature | layer | grammers |
|---|---|---|
| Async / non-blocking | ❌ sync | ✅ Tokio |
| DH key exchange | ✅ | ✅ |
| 2FA / SRP | ✅ | ✅ |
| Session persistence | ✅ | ✅ |
| DC migration | ✅ | ✅ |
| Send message | ✅ | ✅ |
| TL code generation | ✅ custom | ✅ grammers-tl-gen |
| Update / event handling | ❌ | ✅ |
| Media upload/download | ❌ | ✅ |
| Dialogs / contacts | ❌ | ✅ |
| Flood wait handling | ❌ | ✅ |
| MTProxy support | ❌ | ✅ |
| Production ready | ❌ | ✅ |
| Purpose | Learning & experiment | Real projects |

**Use grammers for anything real. Use layer to understand how MTProto actually works.**

---

## 🔧 Feature Flags

`layer-tl-types` exposes optional compile-time features:

| Feature | Default | Description |
|---|---|---|
| `tl-api` | ✅ | High-level Telegram API schema (api.tl) |
| `tl-mtproto` | ❌ | Low-level MTProto schema (mtproto.tl) |
| `impl-debug` | ✅ | `#[derive(Debug)]` on all generated types |
| `impl-from-type` | ✅ | `From<types::T> for enums::E` |
| `impl-from-enum` | ✅ | `TryFrom<enums::E> for types::T` |
| `name-for-id` | ❌ | `name_for_id(u32) -> Option<&'static str>` |
| `impl-serde` | ❌ | `serde::Serialize` / `Deserialize` on all types |
| `deserializable-functions` | ❌ | `Deserializable` on RPC function types (server use) |

```toml
[dependencies]
layer-tl-types = { git = "...", features = ["impl-serde", "name-for-id"] }
```

---

## 📐 Updating to a New TL Layer

1. Replace `layer-tl-types/tl/api.tl` with the new schema file.
2. Update the `// LAYER N` comment on the first line.
3. Run `cargo build` — all types regenerate automatically. That's it.

---

## 🧪 Tests

```bash
cargo test --workspace
```

---

## 📄 License

Licensed under either of, at your option:

- **MIT License** — see [LICENSE-MIT](LICENSE-MIT)
- **Apache License, Version 2.0** — see [LICENSE-APACHE](LICENSE-APACHE)

---

## 👤 Author

**Ankit Chaubey**  
GitHub: [github.com/ankit-chaubey](https://github.com/ankit-chaubey)

---

## 🙏 Acknowledgements

- [**Lonami**](https://codeberg.org/Lonami) — for [grammers](https://codeberg.org/Lonami/grammers).
  The architecture, design decisions, SRP math, and session handling in this project
  are all directly inspired by grammers. It's a fantastic library and an even better
  learning resource. Thank you for making it open source! 🎉

- [**Telegram**](https://core.telegram.org/mtproto) — for the detailed MTProto specification.

---

## 📦 Publishing to crates.io

The publishable crates (in dependency order) are:

```
layer-tl-parser  →  layer-tl-gen  →  layer-tl-types  →  layer-crypto
                                                              ↓
                                                        layer-mtproto
                                                              ↓
                                                        layer-client
                                                              ↓
                                                           layer
```

**One-time setup:**

```bash
# 1. Init git (crates.io requires a clean git repo)
git init
git add .
git commit -m "initial release v0.1.0"

# 2. Login with your crates.io API token
cargo login
```

**Publish in this exact order** — each crate must finish uploading before the next:

```bash
# 1. No layer deps
cargo publish -p layer-tl-parser

# 2. Depends on layer-tl-parser
cargo publish -p layer-tl-gen

# 3. Depends on layer-tl-parser + layer-tl-gen (build-deps)
cargo publish -p layer-tl-types

# 4. No layer deps
cargo publish -p layer-crypto

# 5. Depends on layer-tl-types + layer-crypto
cargo publish -p layer-mtproto

# 6. Depends on layer-tl-types + layer-mtproto
cargo publish -p layer-client

# 7. Facade — depends on everything above
cargo publish -p layer
```

> `layer-app` and `layer-connect` are marked `publish = false` — they are skipped automatically.

**Dry run first** to catch issues without uploading:

```bash
cargo publish -p layer-tl-parser --dry-run
# repeat for each crate
```

**Bump the version** for future releases (all crates share the workspace version):

```bash
# 1. Edit `version = "0.1.0"` → `"0.2.0"` in the root Cargo.toml
# 2. Commit the change
git add Cargo.toml && git commit -m "bump to v0.2.0"
# 3. Publish in the same order above
```

---

*layer — because sometimes you have to build it yourself to truly understand it.*

