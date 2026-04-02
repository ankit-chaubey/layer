<div align="center">

# ⚡ layer

**A modular, production-grade async Rust implementation of the Telegram MTProto protocol.**

[![Crates.io](https://img.shields.io/crates/v/layer-client?color=fc8d62&label=layer-client)](https://crates.io/crates/layer-client)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-2024_edition-f74c00)](https://www.rust-lang.org/)
[![TL Layer](https://img.shields.io/badge/TL%20Layer-224-8b5cf6)](https://core.telegram.org/schema)
[![Build](https://img.shields.io/badge/build-passing-brightgreen)](#)
[![Docs](https://img.shields.io/badge/docs-online-5865F2?style=flat-square&logo=mdbook)](https://github.ankitchaubey.in/layer/)
[![Channel](https://img.shields.io/badge/Telegram-@layer__rs-2CA5E0?logo=telegram)](https://t.me/layer_rs)
[![Chat](https://img.shields.io/badge/Telegram%20Chat-@layer__chat-2CA5E0?logo=telegram)](https://t.me/layer_chat)

*Written from the ground up to understand Telegram's internals at the lowest level.*

</div>

---

## 🧩 What is layer?

**layer** is a hand-crafted, bottom-up async Rust implementation of the
[Telegram MTProto](https://core.telegram.org/mtproto) protocol — built not to
reinvent the wheel, but to *understand* it.

The core protocol pieces — the `.tl` schema parser, the AES-IGE cipher, the
Diffie-Hellman key exchange, the MTProto session, the async update stream — are
all written from scratch. The async runtime and a handful of well-known utilities
(`tokio`, `flate2`, `getrandom`) are borrowed from the ecosystem, because that's
just good engineering. The architecture draws inspiration from the excellent
[grammers](https://codeberg.org/Lonami/grammers) library.

The goal was never "yet another Telegram SDK." It was: *what happens if you sit
down and build every piece yourself, and actually understand why it works?*

> **🎓 Personal use & learning project** — layer was built as a personal exploration:
> *learning by building*. The goal is to deeply understand Telegram's protocol by
> implementing every layer from scratch, not to ship a polished production SDK.
> Feel free to explore, learn from it, or hack on it!

> **⚠️ Pre-production (0.x.x)** — This library is still in early development.
> APIs **will** change without notice. **Not production-ready — use at your own risk!**

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
| [`layer-tl-types`](./layer-tl-types) | All Layer **224** constructors, functions and enums |
| [`layer-crypto`](./layer-crypto) | AES-IGE, RSA, SHA, DH key derivation |
| [`layer-mtproto`](./layer-mtproto) | MTProto session, DH exchange, message framing |
| [`layer-client`](./layer-client) | High-level async client — auth, bots, updates, 2FA, media |
| `layer-app` | Interactive demo binary (not published) |
| `layer-connect` | Raw DH connection demo (not published) |

```
layer/
├── layer-tl-parser/   ── Parses .tl schema text → AST
├── layer-tl-gen/      ── AST → Rust source (build-time)
├── layer-tl-types/    ── Auto-generated types, functions & enums (Layer 224)
├── layer-crypto/      ── AES-IGE, RSA, SHA, auth key derivation
├── layer-mtproto/     ── MTProto session, DH, framing, transport
├── layer-client/      ── High-level async Client API
├── layer-connect/     ── Demo: raw DH + getConfig
└── layer-app/         ── Demo: interactive login + update stream
```

---

## 🆕 What's New in v0.4.4

### Session Backends
- **`StringSessionBackend`** — portable base64-encoded session. Store it in an env var, a DB column, or anywhere — then restore it on the next run:
  ```rust
  // Export
  let s = client.export_session_string().await?;
  // Restore
  let (client, _shutdown) = Client::with_string_session(&s).await?;
  ```
- **`export_session_string()`** — serialises the live session (auth key + DC + peer cache) to a printable string
- **`LibSqlBackend`** — session backend for libsql/Turso databases (`features = ["libsql-session"]`)

### New Update Variants
- **`Update::ChatAction(ChatActionUpdate)`** — fires when a user starts/stops typing, uploading, recording, etc.
- **`Update::UserStatus(UserStatusUpdate)`** — fires when a contact's online status changes

### New Client Method
- **`sync_update_state()`** — forces an immediate `updates.getState` round-trip to reconcile pts/seq after long disconnects

### Bug Fixes (7)
- `iter_participants` no longer silently truncates at 200 members — pagination now correct for large channels/groups
- `GlobalSearchBuilder::fetch` no longer returns duplicate results across page boundaries
- `DownloadIter` last-chunk zero-padding fixed — chunk trimmed to exact file size
- `send_chat_action` with `top_msg_id` (forum topic) no longer panics when used on a basic group
- `answer_inline_query` with an empty result list no longer triggers RPC `400 RESULTS_TOO_MUCH`
- `PossibleGapBuffer` memory leak fixed — buffered updates now released after gap is resolved via `getDifference`
- `ban_participant` with a temporary unix timestamp no longer overflows on 32-bit targets

### Comprehensive Docs Rewrite
- All 25+ documentation pages audited, corrected, and expanded for 0.4.4
- New pages: **Session Backends**, **Search**, **Reactions**, **Admin & Ban Rights**, **Typing Guard**
- `SUMMARY.md` updated with all new sections

> See the full [CHANGELOG](CHANGELOG.md) for complete history including v0.4.0's major additions (MTProto fixes, update engine, search, reactions, admin rights, and more).

---

## 🚀 Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
layer-client = "0.4.4"
tokio = { version = "1", features = ["full"] }
```

### 👤 User Account

```rust
use layer_client::{Client, Config, SignInError};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (client, _shutdown) = Client::connect(Config {
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
                client.check_password(*t, "my_2fa_password").await?;
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
    let (client, _shutdown) = Client::connect(Config {
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

## ✅ What Is Supported

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
- `msg_container` (multi-message) unpacking
- gzip-packed response decompression
- Server salt tracking, pong, `bad_server_salt` handling
- `bad_msg_notification` handling
- Reconnect with exponential backoff + ±20% jitter
- Network hint channel (`signal_network_restored()`) to skip backoff immediately

### 🚂 Transports
- **Abridged** — default, single-byte length prefix (lowest overhead)
- **Intermediate** — 4-byte LE length prefix, better proxy compat
- **Obfuscated2** — XOR stream cipher over Abridged, bypasses DPI / MTProxy

### 📦 TL Type System
- Full `.tl` schema parser
- Build-time Rust code generation
- All **Layer 224** constructors — **2,329** definitions
- `Serializable` / `Deserializable` traits for all types
- `RemoteCall` trait for all RPC functions
- Optional: `Debug`, `serde`, `name_for_id(u32)`

### 👤 Auth
- Phone code login (`request_login_code` → `sign_in`)
- 2FA SRP password (`check_password`)
- Bot token login (`bot_sign_in`)
- Sign out (`sign_out`)
- DC migration — `PHONE_MIGRATE_*`, `USER_MIGRATE_*`, `NETWORK_MIGRATE_*`
- FLOOD_WAIT auto-retry with configurable policy (`RetryPolicy` trait)
- Configurable I/O error retry treated as a flood-equivalent delay

### 💬 Messaging
- Send text message by username/peer (`send_message`, `send_message_to_peer`)
- Send with full options — reply_to, silent, schedule, no_webpage, entities, markup via `InputMessage` builder
- Send to Saved Messages (`send_to_self`)
- Edit message (`edit_message`)
- Edit inline bot message (`edit_inline_message`)
- Forward messages (`forward_messages`)
- Delete messages (`delete_messages`)
- Get messages by ID (`get_messages_by_id`)
- Get / delete scheduled messages
- Pin / unpin message, unpin all messages
- Get pinned message
- Send chat action / typing indicator (`send_chat_action`)
- `TypingGuard` — RAII wrapper, keeps typing alive on an interval, auto-cancels on drop
- Send reactions (`send_reaction`)
- Mark as read, clear mentions

### 📎 Media
- Upload file from bytes — sequential (`upload_file`) and concurrent parallel-parts (`upload_file_concurrent`)
- Upload from async `AsyncRead` stream (`upload_stream`)
- Send file as document or photo (`send_file`)
- Send multiple media in one album (`send_multi_media`)
- Download media to file (`download_media_to_file`)
- Chunked streaming download (`DownloadIter`)
- `Photo`, `Document`, `Sticker` ergonomic wrappers with accessors
- `Downloadable` trait for generic media location handling

### ⌨️ Keyboards & Reply Markup
- `InlineKeyboard` row builder
- Button types: callback, url, url_auth, switch_inline, switch_elsewhere, webview, simple_webview, request_phone, request_geo, request_poll, request_quiz, game, buy, copy_text
- Reply keyboard (standard keyboard) builder
- Answer callback query, answer inline query

### 📋 Text Parsing
- Markdown parser → `(plain_text, Vec<MessageEntity>)`
- HTML parser → `(plain_text, Vec<MessageEntity>)` (feature `html`)
- spec-compliant html5ever tokenizer replacing `parse_html` (feature `html5ever`)
- HTML generator (`generate_html`) always available, hand-rolled

### 👥 Participants & Chat Management
- Get participants, kick, ban with `BanRights` builder, promote with `AdminRightsBuilder`
- Get profile photos
- Search peer members
- Join chat / accept invite link
- Delete dialog

### 🔍 Search
- Search messages in a peer (`SearchBuilder`)
- Global search across all chats (`GlobalSearchBuilder`)

### 📜 Dialogs & Iteration
- List dialogs (`get_dialogs`)
- Lazy dialog iterator (`iter_dialogs`)
- Lazy message iterator per peer (`iter_messages`)

### 🔗 Peer Resolution
- Resolve username → peer (`resolve_username`)
- Resolve peer string (username, phone, `"me"`) → `InputPeer` (`resolve_peer`)
- Access-hash cache, restored from session across restarts

### 💾 Session Persistence
- Binary file session (`BinaryFileBackend`) — default
- In-memory session (`InMemoryBackend`) — testing / ephemeral bots
- SQLite session (`SqliteBackend`) — feature `sqlite`
- `SessionBackend` trait — plug in any custom backend
- Catch-up mode (`Config::catch_up`) — replay missed updates via `getDifference` on reconnect

### 🌐 Networking
- SOCKS5 proxy (`Config::socks5`, optional username/password auth)
- Multi-DC pool — auth keys stored per DC, connections created on demand
- `invoke_on_dc` — send a request to a specific DC
- Raw escape hatch: `client.invoke::<R>()` for any Layer 224 method not yet wrapped

### 🔔 Updates
- Typed update stream (`stream_updates()`)
- `Update::NewMessage` / `MessageEdited` / `MessageDeleted`
- `Update::CallbackQuery` / `InlineQuery` / `InlineSend`
- `Update::Raw` — unmapped TL update passthrough
- PTS / QTS / seq / date gap detection and fill via `getDifference`
- Per-channel PTS tracking

### 🛑 Shutdown
- `ShutdownToken` returned from `Client::connect` — call `.cancel()` to begin graceful shutdown
- `client.disconnect()` — disconnect without token

---

## ❌ What Is NOT Supported

These are high level gaps, not planned omissions, just not built yet.
Use `client.invoke::<R>()` with raw TL types as a workaround for any of these.

| Feature | Notes |
|---|---|
| **Secret chats (E2E)** | MTProto layer-2 secret chats not implemented |
| **Voice / video calls** | No call signalling or media transport |
| **Payments** | `SentCode::PaymentRequired` returns an error; payment flow not implemented |
| **Group / channel creation** | `createChat`, `createChannel` not wrapped |
| **Channel admin tooling** | No channel creation, migration, linking, or statistics — admin/ban rights are supported via `set_admin_rights` / `set_banned_rights` |
| **Sticker set management** | No `getStickerSet`, `uploadStickerFile`, etc. |
| **Account settings** | No profile update, privacy settings, notification preferences |
| **Contact management** | No `importContacts`, `deleteContacts` |
| **Poll / quiz creation** | No `InputMediaPoll` wrapper |
| **Live location** | Not wrapped |
| **Bot menu / command registration** | `setMyCommands`, `setBotInfo` not wrapped |
| **IPv6** | Config flag exists (`allow_ipv6: false`) but not tested |
| **MTProxy with proxy secret** | Obfuscated2 transport works; MTProxy secret mode untested |

---

## 🔧 Feature Flags

### `layer-tl-types`

| Feature | Default | Description |
|---|---|---|
| `tl-api` | ✅ | High-level Telegram API schema |
| `tl-mtproto` | ❌ | Low-level MTProto schema |
| `impl-debug` | ✅ | `#[derive(Debug)]` on generated types |
| `impl-from-type` | ✅ | `From<types::T> for enums::E` |
| `impl-from-enum` | ✅ | `TryFrom<enums::E> for types::T` |
| `name-for-id` | ❌ | `name_for_id(u32) -> Option<&'static str>` |
| `impl-serde` | ❌ | `serde::Serialize` / `Deserialize` |

### `layer-client`

| Feature | Default | Description |
|---|---|---|
| `html` | ❌ | Built-in hand-rolled HTML parser (`parse_html`, `generate_html`) |
| `html5ever` | ❌ | spec-compliant html5ever tokenizer replaces `parse_html` |
| `sqlite` | ❌ | SQLite session backend (`SqliteBackend`) |

---

## 📐 Updating to a New TL Layer

```bash
# 1. Replace the schema
cp new-api.tl layer-tl-types/tl/api.tl

# 2. Build — types regenerate automatically
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

- **MIT License** — see [LICENSE-MIT](LICENSE-MIT)
- **Apache License, Version 2.0** — see [LICENSE-APACHE](LICENSE-APACHE)

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

- The Rust async ecosystem — `tokio`, `getrandom`, `flate2`, and friends.

- 🤖 AI tools used for clearer documentation and better comments across the repo
  (2026 is a good year to use AI).  
  Even regrets 😁 after making these docs through AI. iykyk.
  Too lazy to revert and type again, so it stays as is!

---

## ⚠️ Telegram Terms of Service

As with any third-party Telegram library, please ensure that your usage
complies with [Telegram's Terms of Service](https://core.telegram.org/api/terms).
Misuse or abuse of the Telegram API may result in temporary limitations or
permanent bans of Telegram accounts.

---

<div align="center">

*layer.. because sometimes you have to build it yourself to truly understand it.*

</div>
