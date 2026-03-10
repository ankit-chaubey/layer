<div align="center">

# ⚡ layer

**A modular, production-grade async Rust implementation of the Telegram MTProto protocol.**

[![Crates.io](https://img.shields.io/crates/v/layer-client?color=fc8d62&label=layer-client)](https://crates.io/crates/layer-client)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-2024_edition-f74c00)](https://www.rust-lang.org/)
[![TL Layer](https://img.shields.io/badge/TL%20Layer-223-8b5cf6)](https://core.telegram.org/schema)
[![Build](https://img.shields.io/badge/build-passing-brightgreen)](#)
[![Docs](https://img.shields.io/badge/docs-online-5865F2?style=flat-square&logo=mdbook)](https://github.ankitchaubey.in/layer/)
[![Channel](https://img.shields.io/badge/Telegram-@layer__rs-2CA5E0?logo=telegram)](https://t.me/layer_rs)
[![Chat](https://img.shields.io/badge/Telegram%20Chat-@layer__chat-2CA5E0?logo=telegram)](https://t.me/layer_chat)

*Written from the ground up to understand Telegram's internals at the lowest level.*

</div>

---

## 🧩 What is layer?

**layer** is a hand-crafted, bottom-up async Rust implementation of the
[Telegram MTProto](https://core.telegram.org/mtproto) protocol - built not to
reinvent the wheel, but to *understand* it.

The core protocol pieces - the `.tl` schema parser, the AES-IGE cipher, the
Diffie-Hellman key exchange, the MTProto session, the async update stream - are
all written from scratch. The async runtime and a handful of well-known utilities
(`tokio`, `flate2`, `getrandom`) are borrowed from the ecosystem, because that's
just good engineering. The architecture draws inspiration from the excellent
[grammers](https://codeberg.org/Lonami/grammers) library.

The goal was never "yet another Telegram SDK." It was: *what happens if you sit
down and build every piece yourself, and actually understand why it works?*

> **🎓 Personal use & learning project** - layer was built as a personal exploration:
> *learning by building*. The goal is to deeply understand Telegram's protocol by
> implementing every layer from scratch, not to ship a polished production SDK.
> Feel free to explore, learn from it, or hack on it!

> **⚠️ Pre-production (0.x.x)** - This library is still in early development.
> APIs **will** change without notice. **Not production-ready - use at your own risk!**

---

## 💬 Community

| | Link |
|---|---|
| 📢 **Channel** (updates & releases) | [@layer_rs](https://t.me/layer_rs) |
| 💬 **Chat** (questions & discussion) | [@layer_chat](https://t.me/layer_chat) |

---

## 🏗️ Crate Overview

| Crate | Description |
|---|---|
| [`layer-tl-parser`](./layer-tl-parser) | Parses `.tl` schema text into an AST |
| [`layer-tl-gen`](./layer-tl-gen) | Generates Rust code from the AST at build time |
| [`layer-tl-types`](./layer-tl-types) | All Layer 223 constructors, functions and enums |
| [`layer-crypto`](./layer-crypto) | AES-IGE, RSA, SHA, DH key derivation |
| [`layer-mtproto`](./layer-mtproto) | MTProto session, DH exchange, message framing |
| [`layer-client`](./layer-client) | High-level async client - auth, bots, updates, 2FA |
| `layer-app` | Interactive demo binary (not published) |
| `layer-connect` | Raw DH connection demo (not published) |

```
layer/
├── layer-tl-parser/   ── Parses .tl schema text → AST
├── layer-tl-gen/      ── AST → Rust source (build-time)
├── layer-tl-types/    ── Auto-generated types, functions & enums (Layer 223)
├── layer-crypto/      ── AES-IGE, RSA, SHA, auth key derivation
├── layer-mtproto/     ── MTProto session, DH, framing, transport
├── layer-client/      ── High-level async Client API
├── layer-connect/     ── Demo: raw DH + getConfig
└── layer-app/         ── Demo: interactive login + update stream
```

---

## 🚀 Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
layer-client = "0.2.2"
tokio = { version = "1", features = ["full"] }
```

### 👤 User Account

```rust
use layer_client::{Client, Config, SignInError};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::connect(Config {
        session_path: "my.session".into(),
        api_id:       12345,          // https://my.telegram.org
        api_hash:     "abc123".into(),
        ..Default::default()
    }).await?;

    if !client.is_authorized().await? {
        let token = client.request_login_code("+1234567890").await?;
        let code  = "12345"; // read from stdin

        match client.sign_in(&token, code).await {
            Ok(name) => println!("Welcome, {name}!"),
            Err(SignInError::PasswordRequired(t)) => {
                client.check_password(t, "my_2fa_password").await?;
            }
            Err(e) => return Err(e.into()),
        }
        client.save_session().await?;
    }

    client.send_message("me", "Hello from layer! 👋").await?;
    Ok(())
}
```

### 🤖 Bot

```rust
use layer_client::{Client, Config, update::Update};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::connect(Config {
        session_path: "bot.session".into(),
        api_id:       12345,
        api_hash:     "abc123".into(),
        ..Default::default()
    }).await?;

    client.bot_sign_in("1234567890:ABCdef...").await?;
    client.save_session().await?;

    let mut updates = client.stream_updates();
    while let Some(update) = updates.next().await {
        match update {
            Update::NewMessage(msg) if !msg.outgoing() => {
                if let Some(peer) = msg.peer_id() {
                    client.send_message_to_peer(
                        peer.clone(),
                        &format!("Echo: {}", msg.text().unwrap_or("")),
                    ).await?;
                }
            }
            Update::CallbackQuery(cb) => {
                client.answer_callback_query(cb.query_id, Some("Done!"), false).await?;
            }
            _ => {}
        }
    }
    Ok(())
}
```

---

## ✅ Features

### 🔐 Cryptography
- AES-IGE encryption / decryption (MTProto 2.0)
- RSA encryption with Telegram's public keys
- SHA-1 and SHA-256 hashing
- Auth key derivation from DH nonce material
- PQ factorization (Pollard's rho)
- Diffie-Hellman shared secret computation

### 📡 MTProto
- Full 3-step DH key exchange handshake
- MTProto 2.0 encrypted sessions
- Proper message framing (salt, session_id, msg_id, seq_no)
- Abridged TCP transport
- `msg_container` (multi-message) unpacking
- gzip-packed response decompression
- Server salt tracking, pong, bad_server_salt handling

### 📦 TL Type System
- Full `.tl` schema parser
- Build-time Rust code generation
- All Layer 223 constructors - 2,295 definitions
- `Serializable` / `Deserializable` traits for all types
- `RemoteCall` trait for all RPC functions
- Optional: `Debug`, `serde`, `name_for_id(u32)`

### 👤 Client
- `Client::connect()` - async TCP + DH + initConnection
- Session persistence across restarts
- Phone code login + 2FA SRP
- Bot token login
- DC migration (PHONE_MIGRATE, USER_MIGRATE)
- FLOOD_WAIT auto-retry with configurable policy
- Async update stream with typed events
- Send / delete / fetch messages
- Dialogs list
- Username / peer resolution
- Raw `RemoteCall` escape hatch for any API method

---

## 🔧 Feature Flags (`layer-tl-types`)

| Feature | Default | Description |
|---|---|---|
| `tl-api` | ✅ | High-level Telegram API schema |
| `tl-mtproto` | ❌ | Low-level MTProto schema |
| `impl-debug` | ✅ | `#[derive(Debug)]` on generated types |
| `impl-from-type` | ✅ | `From<types::T> for enums::E` |
| `impl-from-enum` | ✅ | `TryFrom<enums::E> for types::T` |
| `name-for-id` | ❌ | `name_for_id(u32) -> Option<&'static str>` |
| `impl-serde` | ❌ | `serde::Serialize` / `Deserialize` |

---

## 📐 Updating to a New TL Layer

```bash
# 1. Replace the schema
cp new-api.tl layer-tl-types/tl/api.tl

# 2. Build - types regenerate automatically
cargo build
```

---

## 🧪 Tests

```bash
cargo test --workspace
```

---

## 📄 License

Licensed under either of, at your option:

- **MIT License** - see [LICENSE-MIT](LICENSE-MIT)
- **Apache License, Version 2.0** - see [LICENSE-APACHE](LICENSE-APACHE)

---

## 👤 Author

<div align="center">

### Ankit Chaubey

*Built with curiosity, caffeine, and a lot of Rust compiler errors. 🦀*

| | |
|:---:|:---|
| 🐙 **GitHub** | [github.com/ankit-chaubey](https://github.com/ankit-chaubey) |
| 🌐 **Website** | [ankitchaubey.in](https://ankitchaubey.in) |
| 📬 **Email** | [ankitchaubey.dev@gmail.com](mailto:ankitchaubey.dev@gmail.com) |
| 📦 **Project** | [github.com/ankit-chaubey/layer](https://github.com/ankit-chaubey/layer) |
| 📢 **Channel** | [@layer_rs](https://t.me/layer_rs) |
| 💬 **Chat** | [@layer_chat](https://t.me/layer_chat) |

</div>

---

## 🙏 Acknowledgements

- [**Lonami**](https://codeberg.org/Lonami) for
  [**grammers**](https://codeberg.org/Lonami/grammers).
  Portions of this project include code derived from the grammers project,
  which is dual-licensed under the MIT or Apache-2.0 licenses. The architecture,
  design decisions, SRP math, and session handling are deeply inspired by grammers.
  It's a fantastic library and an even better learning resource. Thank you for
  making it open source! 🎉

- [**Telegram**](https://core.telegram.org/mtproto) for the detailed MTProto specification.

- The Rust async ecosystem `tokio`, `getrandom`, `flate2`, and friends.

- 🤖 AI tools used for clearer documentation and better comments across the repo
  (2026 is a good year to use AI).  
  Even regrets 😁 after making these docs through AI. iykyk.
  Too lazy to revert and type again, so it stays as is!
  
---

## ⚠️ Telegram Terms of Service

As with any third-party Telegram library, please ensure that your usage
complies with [Telegram’s Terms of Service](https://core.telegram.org/api/terms).
Misuse or abuse of the Telegram API may result in temporary limitations or
permanent bans of Telegram accounts.

---

<div align="center">

*layer.. because sometimes you have to build it yourself to truly understand it.*

</div>
