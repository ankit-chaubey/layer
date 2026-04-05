<div align="center">

<img src="https://raw.githubusercontent.com/ankit-chaubey/layer/main/docs/images/layer-banner-dark.png" alt="layer — Async Rust MTProto" width="100%" />

<br/>

# ⚡ layer

**A modular, production-grade async Rust library for the Telegram MTProto protocol.**

*Developed By* **[Ankit Chaubey](https://github.com/ankit-chaubey)**

*Built with curiosity, caffeine, and a lot of Rust compiler errors 🦀*

<br/>

[![GitHub](https://img.shields.io/badge/GitHub-ankit--chaubey-181717?style=for-the-badge&logo=github)](https://github.com/ankit-chaubey)
[![Website](https://img.shields.io/badge/Website-ankitchaubey.in-10b981?style=for-the-badge&logo=safari)](https://ankitchaubey.in)

<br/>

[![Crates.io](https://img.shields.io/crates/v/layer-client?style=for-the-badge&color=fc8d62&label=layer-client&logo=rust)](https://crates.io/crates/layer-client)
[![Downloads](https://img.shields.io/crates/d/layer-client?style=for-the-badge&color=f59e0b&logo=rust&label=downloads)](https://crates.io/crates/layer-client)
[![docs.rs](https://img.shields.io/badge/docs.rs-layer--client-5865F2?style=for-the-badge&logo=docs.rs)](https://docs.rs/layer-client)
[![Guide](https://img.shields.io/badge/book-online%20guide-10b981?style=for-the-badge&logo=mdbook)](https://layer.ankitchaubey.in/)

<br/>

[![License](https://img.shields.io/badge/license-MIT%20%7C%20Apache--2.0-blue?style=flat-square)](LICENSE-MIT)
[![Rust 2024](https://img.shields.io/badge/rust-2024%20edition-f74c00?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![TL Layer](https://img.shields.io/badge/TL%20Layer-224-8b5cf6?style=flat-square)](https://core.telegram.org/schema)
[![Tokio](https://img.shields.io/badge/async-tokio-6366f1?style=flat-square)](https://tokio.rs)
[![Build](https://img.shields.io/badge/build-passing-22c55e?style=flat-square)](https://github.com/ankit-chaubey/layer/actions)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen?style=flat-square)](CONTRIBUTING.md)

<br/>

[![Telegram Channel](https://img.shields.io/badge/channel-%40layer__rs-2CA5E0?style=for-the-badge&logo=telegram)](https://t.me/layer_rs)
[![Telegram Chat](https://img.shields.io/badge/chat-%40layer__chat-2CA5E0?style=for-the-badge&logo=telegram)](https://t.me/layer_chat)

</div>

<br/>

> **Pre-production (`0.x.x`)** — APIs may change between minor versions. Review the [CHANGELOG](CHANGELOG.md) before upgrading.

<br/>

---

## Table of Contents

- [What is layer?](#-what-is-layer)
- [What makes layer unique?](#-what-makes-layer-unique)
- [Crate Overview](#-crate-overview)
- [Installation](#-installation)
- [The Minimal Bot — 15 Lines](#-the-minimal-bot--15-lines)
- [Quick Start — User Account](#-quick-start--user-account)
- [Quick Start — Bot](#-quick-start--bot)
  - [Spawning per-update tasks](#spawning-per-update-tasks)
- [ClientBuilder](#-clientbuilder)
- [String Sessions — Portable Auth](#-string-sessions--portable-auth)
- [Update Stream](#-update-stream)
  - [Update variants](#update-variants)
  - [IncomingMessage API](#incomingmessage-api)
- [Messaging](#-messaging)
  - [Send text](#send-text)
  - [InputMessage builder](#inputmessage-builder)
  - [Edit, forward, delete](#edit-forward-delete)
  - [Pin and unpin](#pin-and-unpin)
  - [Scheduled messages](#scheduled-messages)
  - [Chat actions and typing](#chat-actions-and-typing)
- [Media](#-media)
  - [Upload](#upload)
  - [Download](#download)
- [Keyboards and Reply Markup](#-keyboards-and-reply-markup)
  - [Inline keyboards](#inline-keyboards)
  - [Reply keyboards](#reply-keyboards)
  - [Answer callback queries](#answer-callback-queries)
  - [Inline mode](#inline-mode)
- [Text Formatting](#-text-formatting)
  - [Markdown](#markdown)
  - [HTML](#html)
- [Reactions](#-reactions)
- [Typing Guard (RAII)](#-typing-guard-raii)
- [Participants and Chat Management](#-participants-and-chat-management)
  - [Fetch participants](#fetch-participants)
  - [Ban, kick, promote](#ban-kick-promote)
  - [Profile photos](#profile-photos)
- [Search](#-search)
  - [In-chat search](#in-chat-search)
  - [Global search](#global-search)
- [Dialogs and Iterators](#-dialogs-and-iterators)
- [Peer Resolution](#-peer-resolution)
- [Session Backends](#-session-backends)
- [Feature Flags](#-feature-flags)
- [Raw API Escape Hatch](#-raw-api-escape-hatch)
- [Transports](#-transports)
- [Networking — SOCKS5 and DC Pool](#-networking--socks5-and-dc-pool)
- [Error Handling](#-error-handling)
- [Shutdown](#-shutdown)
- [Updating the TL Layer](#-updating-the-tl-layer)
- [Running Tests](#-running-tests)
- [Unsupported Features](#-unsupported-features)
- [Community](#-community)
- [Contributing](#-contributing)
- [Security](#-security)
- [Author](#-author)
- [Acknowledgements](#-acknowledgements)
- [License](#-license)
- [Telegram Terms of Service](#%EF%B8%8F-telegram-terms-of-service)

<br/>

---

## 🧩 What is layer?

**layer** is a hand-crafted, bottom-up async Rust implementation of the [Telegram MTProto](https://core.telegram.org/mtproto) protocol.

Every core piece — the `.tl` schema parser, the AES-IGE cipher, the Diffie-Hellman key exchange, the MTProto session, the async typed update stream — is written from scratch, owned by this project, and fully understood. The async runtime and a handful of well-known utilities (`tokio`, `flate2`, `getrandom`) come from the ecosystem, because that's good engineering.

The goal was never *"yet another Telegram SDK."* It was: **what happens if you sit down and build every piece yourself, and truly understand why it works?**

<br/>

---

## 💡 What makes layer unique?

Most Telegram libraries are thin wrappers around generated code or ports from other languages. layer is different.

**Built from first principles.** The `.tl` schema parser, the AES-IGE cipher, the Diffie-Hellman key exchange, and the MTProto framing are all implemented from scratch — not borrowed from a C++ library or wrapped behind FFI. Every algorithm is understood and owned by this project.

**Modular workspace architecture.** layer is not a monolith. Each concern lives in its own focused crate: schema parsing, code generation, cryptographic primitives, the protocol session, and the high-level client are all separate, versioned, independently usable pieces.

**A full escape hatch.** Every one of Telegram's 2,329 Layer 224 API methods is accessible via `client.invoke()` with the fully-typed TL schema — even if no high-level wrapper exists yet. You never hit a wall.

**Unique session flexibility.** layer ships with binary file, in-memory, string (base64), SQLite, and libsql/Turso session backends out of the box — and supports custom `SessionBackend` implementations for any other storage (Redis, Postgres, S3, etc.).

**Android / Termux tested.** The reconnect logic, backoff parameters, and socket handling are tuned for mobile conditions. layer is actively developed and tested on Android via Termux.

**No `unsafe`, pure async Rust.** The entire stack from cryptographic primitives to the high-level client is safe Rust, running on Tokio.

<br/>

<div align="center">
<img src="https://raw.githubusercontent.com/ankit-chaubey/layer/main/docs/images/arch-stack.svg" alt="layer crate architecture" width="100%"/>
</div>

<br/>

---

## 🏗️ Crate Overview

<div align="center">
<img src="https://raw.githubusercontent.com/ankit-chaubey/layer/main/docs/images/feature-flags.svg" alt="Feature flags" width="100%"/>
</div>

<br/>

layer is a workspace of focused crates. Most users only ever need **`layer-client`**.

| Crate | Version | Description |
|---|:---:|---|
| [`layer-client`](./layer-client) | [![crates.io](https://img.shields.io/crates/v/layer-client?style=flat-square&color=fc8d62)](https://crates.io/crates/layer-client) | High-level async client: auth, send, receive, media, bots |
| [`layer-tl-types`](./layer-tl-types) | [![crates.io](https://img.shields.io/crates/v/layer-tl-types?style=flat-square&color=f59e0b)](https://crates.io/crates/layer-tl-types) | All Layer **224** constructors, functions, and enums (2,329 definitions) |
| [`layer-mtproto`](./layer-mtproto) | [![crates.io](https://img.shields.io/crates/v/layer-mtproto?style=flat-square&color=6366f1)](https://crates.io/crates/layer-mtproto) | MTProto session, DH exchange, message framing, transports |
| [`layer-crypto`](./layer-crypto) | [![crates.io](https://img.shields.io/crates/v/layer-crypto?style=flat-square&color=8b5cf6)](https://crates.io/crates/layer-crypto) | AES-IGE, RSA, SHA, Diffie-Hellman, auth key derivation |
| [`layer-tl-gen`](./layer-tl-gen) | [![crates.io](https://img.shields.io/crates/v/layer-tl-gen?style=flat-square&color=10b981)](https://crates.io/crates/layer-tl-gen) | Build-time Rust code generator from the TL AST |
| [`layer-tl-parser`](./layer-tl-parser) | [![crates.io](https://img.shields.io/crates/v/layer-tl-parser?style=flat-square&color=22c55e)](https://crates.io/crates/layer-tl-parser) | Parses `.tl` schema text into an AST |
| `layer-app` | ❌ | Interactive demo binary (not published) |
| `layer-connect` | ❌ | Raw DH connection demo (not published) |

```
layer/
├── layer-tl-parser/      .tl schema text → AST
├── layer-tl-gen/         AST → Rust source (build-time codegen)
├── layer-tl-types/       Auto-generated types, functions & enums (Layer 224)
├── layer-crypto/         AES-IGE, RSA, SHA, auth key derivation, PQ factorization
├── layer-mtproto/        MTProto session, DH handshake, framing, transport
├── layer-client/         High-level async Client API  ← you are here
├── layer-connect/        Demo: raw DH + getConfig
└── layer-app/            Demo: interactive login + update stream
```

> The full API reference lives at **[docs.rs/layer-client](https://docs.rs/layer-client)**.
> The narrative guide lives at **[layer.ankitchaubey.in](https://layer.ankitchaubey.in/)**.

<br/>

---

## 📦 Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
layer-client = "0.4.6"
tokio        = { version = "1", features = ["full"] }
```

Get your `api_id` and `api_hash` from **[my.telegram.org](https://my.telegram.org)** — every Telegram client needs them.

**Optional feature flags:**

```toml
# SQLite session persistence (stores auth key in a local .db file)
layer-client = { version = "0.4.6", features = ["sqlite-session"] }

# libsql / Turso remote or embedded database session
layer-client = { version = "0.4.6", features = ["libsql-session"] }

# Hand-rolled HTML entity parser (parse_html / generate_html)
layer-client = { version = "0.4.6", features = ["html"] }

# Spec-compliant html5ever tokenizer — replaces the built-in html parser
layer-client = { version = "0.4.6", features = ["html5ever"] }
```

> **Note:** `layer-client` re-exports `layer_tl_types` as `layer_client::tl`, so you usually do not need to add `layer-tl-types` as a direct dependency.

<br/>

---

## ⚡ The Minimal Bot — 15 Lines

This is the least code you need to have a working, update-receiving Telegram bot running with layer.

```rust
use layer_client::{Client, Config, update::Update};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (client, _shutdown) = Client::connect(Config {
        session_path: "bot.session".into(),
        api_id:   std::env::var("API_ID")?.parse()?,
        api_hash: std::env::var("API_HASH")?,
        ..Default::default()
    }).await?;

    client.bot_sign_in(&std::env::var("BOT_TOKEN")?).await?;
    client.save_session().await?;

    let mut stream = client.stream_updates();
    while let Some(Update::NewMessage(msg)) = stream.next().await {
        if let (false, Some(text), Some(peer)) = (msg.outgoing(), msg.text(), msg.peer_id()) {
            client.send_message_to_peer(peer.clone(), &format!("Echo: {text}")).await?;
        }
    }
    Ok(())
}
```

No trait objects, no callbacks, no `dyn Handler`. Just an async loop and pattern matching. That's the whole bot.

> [📖 Read more in the Bot Quick Start guide →](https://layer.ankitchaubey.in/quickstart-bot.html)

<br/>

---

## 👤 Quick Start — User Account

```rust
use layer_client::{Client, Config, SignInError};
use std::io::{self, BufRead};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (client, _shutdown) = Client::connect(Config {
        session_path: "my.session".into(),
        api_id:       12345,
        api_hash:     "your_api_hash".into(),
        ..Default::default()
    })
    .await?;

    if !client.is_authorized().await? {
        let phone = "+1234567890";
        let token = client.request_login_code(phone).await?;

        print!("Enter code: ");
        let stdin = io::stdin();
        let code  = stdin.lock().lines().next().unwrap()?;

        match client.sign_in(&token, &code).await {
            Ok(name) => println!("Welcome, {name}!"),
            Err(SignInError::PasswordRequired(t)) => {
                // 2FA — read password and call check_password
                client.check_password(*t, "my_2fa_password").await?;
            }
            Err(e) => return Err(e.into()),
        }
        client.save_session().await?;
    }

    let me = client.get_me().await?;
    println!("Logged in as: {}", me.first_name.unwrap_or_default());

    // Send a message to Saved Messages
    client.send_message("me", "Hello from layer! 👋").await?;

    // Or send to any peer
    client.send_message_to_peer("@username", "Hello!").await?;

    Ok(())
}
```

> After the first successful login the session is persisted to `my.session`. Subsequent runs skip the phone/code flow entirely.

> [📖 Full user account guide →](https://layer.ankitchaubey.in/quickstart-user.html)

<br/>

---

## 🤖 Quick Start — Bot

```rust
use layer_client::{Client, Config, update::Update};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (client, _shutdown) = Client::connect(Config {
        session_path: "bot.session".into(),
        api_id:       12345,
        api_hash:     "your_api_hash".into(),
        ..Default::default()
    })
    .await?;

    if !client.is_authorized().await? {
        client.bot_sign_in("1234567890:ABCdef...").await?;
        client.save_session().await?;
    }

    let me = client.get_me().await?;
    println!("@{} is online", me.username.as_deref().unwrap_or("bot"));

    let mut stream = client.stream_updates();
    while let Some(update) = stream.next().await {
        match update {
            Update::NewMessage(msg) if !msg.outgoing() => {
                if let Some(peer) = msg.peer_id() {
                    client
                        .send_message_to_peer(
                            peer.clone(),
                            &format!("You said: {}", msg.text().unwrap_or("")),
                        )
                        .await?;
                }
            }
            Update::CallbackQuery(cb) => {
                client
                    .answer_callback_query(cb.query_id, Some("✅ Done!"), false)
                    .await?;
            }
            _ => {}
        }
    }
    Ok(())
}
```

### Spawning per-update tasks

For production bots the update loop should never block. Spawn each update into its own task:

```rust
use layer_client::{Client, update::Update};
use std::sync::Arc;

// Wrap in Arc so it can be moved into spawned tasks
let client = Arc::new(client);
let mut stream = client.stream_updates();

while let Some(update) = stream.next().await {
    let c = client.clone();
    tokio::spawn(async move {
        if let Err(e) = handle_update(update, &c).await {
            eprintln!("handler error: {e}");
        }
    });
}

async fn handle_update(
    update: Update,
    client: &Client,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match update {
        Update::NewMessage(msg) if !msg.outgoing() => {
            if let Some(peer) = msg.peer_id() {
                client.send_message_to_peer(peer.clone(), "👋").await?;
            }
        }
        _ => {}
    }
    Ok(())
}
```

> [📖 Full production bot guide →](https://layer.ankitchaubey.in/quickstart-bot.html)

<br/>

---

## 🔨 ClientBuilder

The fluent [`ClientBuilder`](./layer-client/src/builder.rs) is the cleanest way to configure a connection when you need more than defaults:

```rust
use layer_client::Client;

let (client, _shutdown) = Client::builder()
    .api_id(12345)
    .api_hash("your_api_hash")
    .session("my.session")          // BinaryFileBackend at this path
    .catch_up(true)                 // replay missed updates on reconnect
    .connect()
    .await?;
```

Use `.session_string(s)` for portable base64 sessions (no file on disk):

```rust
let session = std::env::var("SESSION").unwrap_or_default();

let (client, _shutdown) = Client::builder()
    .api_id(12345)
    .api_hash("your_api_hash")
    .session_string(session)
    .connect()
    .await?;
```

Use `.socks5(host, port)` for a proxy:

```rust
let (client, _shutdown) = Client::builder()
    .api_id(12345)
    .api_hash("your_api_hash")
    .session("proxy.session")
    .socks5("127.0.0.1", 1080)
    .connect()
    .await?;
```

> [📖 ClientBuilder reference →](https://docs.rs/layer-client/latest/layer_client/builder/struct.ClientBuilder.html)

<br/>

---

## 🔑 String Sessions — Portable Auth

A string session encodes the entire auth state (auth key, DC, peer cache) into a single printable base64 string. Store it in an environment variable, a database column, a secret manager — anywhere.

```rust
// ── Export from any running client ────────────────────────────────────────────
let session_string = client.export_session_string().await?;
println!("{session_string}");  // save this somewhere safe

// ── Restore later — no phone/code needed ─────────────────────────────────────
let (client, _shutdown) = Client::with_string_session(&session_string).await?;

// Or via builder
let (client, _shutdown) = Client::builder()
    .api_id(12345)
    .api_hash("your_api_hash")
    .session_string(session_string)
    .connect()
    .await?;
```

String sessions are ideal for serverless deployments, CI/CD bots, and any environment where writing files is inconvenient.

> [📖 Session backends guide →](https://layer.ankitchaubey.in/authentication/session-backends.html)

<br/>

---

## 📡 Update Stream

[`client.stream_updates()`](https://docs.rs/layer-client/latest/layer_client/struct.Client.html#method.stream_updates) returns an [`UpdateStream`](https://docs.rs/layer-client/latest/layer_client/struct.UpdateStream.html) that yields typed updates:

```rust
let mut stream = client.stream_updates();
while let Some(update) = stream.next().await {
    // ...
}
```

`stream_updates()` is cheap and can be called multiple times. Each call returns an independent receiver. Use `Arc<Client>` and clone it into spawned tasks.

### Update variants

```rust
use layer_client::update::Update;

match update {
    // ── Messages ──────────────────────────────────────────────────────────
    Update::NewMessage(msg)     => { /* new incoming message */ }
    Update::MessageEdited(msg)  => { /* existing message was edited */ }
    Update::MessageDeleted(del) => { /* one or more messages were deleted */ }

    // ── Bot interactions ──────────────────────────────────────────────────
    Update::CallbackQuery(cb)   => { /* inline button was pressed */ }
    Update::InlineQuery(iq)     => { /* @bot query in inline mode */ }
    Update::InlineSend(is)      => { /* user selected an inline result */ }

    // ── Presence ──────────────────────────────────────────────────────────
    Update::UserTyping(action)  => { /* typing / uploading / recording */ }
    Update::UserStatus(status)  => { /* contact went online / offline */ }

    // ── Raw passthrough ───────────────────────────────────────────────────
    Update::Raw(raw)            => { /* any unmapped TL update */ }

    _ => {}  // Update is #[non_exhaustive] — always add a fallback
}
```

> **Important:** `Update` is `#[non_exhaustive]`. Always include `_ => {}` to stay forward-compatible as new variants are added.

### IncomingMessage API

[`IncomingMessage`](https://docs.rs/layer-client/latest/layer_client/update/struct.IncomingMessage.html) is the type of `NewMessage` and `MessageEdited`:

```rust
Update::NewMessage(msg) => {
    msg.id()          // i32 — unique message ID in the chat
    msg.text()        // Option<&str> — text or caption
    msg.peer_id()     // Option<&tl::enums::Peer> — the chat this message is in
    msg.sender_id()   // Option<&tl::enums::Peer> — who sent it
    msg.outgoing()    // bool — was this sent by us?
    msg.date()        // i32 — Unix timestamp
    msg.edit_date()   // Option<i32> — last edit timestamp
    msg.mentioned()   // bool — are we mentioned?
    msg.silent()      // bool — no notification?
    msg.pinned()      // bool — is the message currently pinned?
    msg.post()        // bool — is this a channel post (no sender)?
    msg.raw           // tl::enums::Message — full TL object for everything else
}
```

> [📖 Incoming message reference →](https://layer.ankitchaubey.in/updates/incoming-message.html)

<br/>

---

## 💬 Messaging

### Send text

The simplest send methods accept any `impl Into<PeerRef>` — a `&str` username, `"me"` for Saved Messages, a `tl::enums::Peer` clone, or a numeric ID:

```rust
// By username
client.send_message("@username", "Hello!").await?;

// To Saved Messages
client.send_message("me", "Note to self").await?;

// By TL Peer (from an incoming message)
if let Some(peer) = msg.peer_id() {
    client.send_message_to_peer(peer.clone(), "Reply!").await?;
}

// To self — shorthand for "me"
client.send_to_self("Reminder: buy milk 🥛").await?;
```

### InputMessage builder

[`InputMessage`](https://docs.rs/layer-client/latest/layer_client/struct.InputMessage.html) gives you full control over every send option:

```rust
use layer_client::{InputMessage, parsers::parse_markdown};
use layer_client::keyboard::InlineKeyboard;

let (text, entities) = parse_markdown("**Bold** and `code`");

let kb = InlineKeyboard::new()
    .row()
    .callback("✅ Confirm", b"confirm")
    .url("🔗 Docs", "https://docs.rs/layer-client")
    .build();

client
    .send_message_to_peer_ex(
        peer.clone(),
        &InputMessage::text(text)
            .entities(entities)         // formatted text
            .reply_to(Some(msg_id))     // reply to a specific message
            .silent(true)               // no notification
            .no_webpage(true)           // suppress link preview
            .keyboard(kb),              // attach inline keyboard
    )
    .await?;
```

### Edit, forward, delete

```rust
// Edit
client.edit_message(peer.clone(), message_id, "Updated text").await?;

// Forward messages between peers
client.forward_messages(
    from_peer.clone(),
    to_peer.clone(),
    &[message_id_1, message_id_2],
).await?;

// Delete (also removes from the other side if you have permission)
client.delete_messages(peer.clone(), &[message_id]).await?;
```

### Pin and unpin

```rust
// Pin a message (notify: true sends a "pinned message" service message)
client.pin_message(peer.clone(), message_id, true).await?;

// Get the current pinned message
let pinned = client.get_pinned_message(peer.clone()).await?;

// Unpin a specific message
client.unpin_message(peer.clone(), message_id).await?;

// Unpin all at once
client.unpin_all_messages(peer.clone()).await?;
```

### Scheduled messages

```rust
use std::time::{SystemTime, UNIX_EPOCH};

// Schedule for 1 hour from now
let schedule_ts = (SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap()
    .as_secs() + 3600) as i32;

client
    .send_message_to_peer_ex(
        peer.clone(),
        &InputMessage::text("Reminder! ⏰").schedule_date(Some(schedule_ts)),
    )
    .await?;

// List all scheduled messages in a chat
let scheduled = client.get_scheduled_messages(peer.clone()).await?;

// Cancel a scheduled message
client.delete_scheduled_messages(peer.clone(), &[scheduled_msg_id]).await?;
```

### Chat actions and typing

```rust
use layer_tl_types as tl;

// Start a "typing..." indicator
client.send_chat_action(
    peer.clone(),
    tl::enums::SendMessageAction::SendMessageTypingAction,
    None,  // top_msg_id — None for normal chats, Some(id) for forum topics
).await?;

// Mark all messages as read
client.mark_as_read(peer.clone()).await?;

// Clear all @mention badges
client.clear_mentions(peer.clone()).await?;
```

> [📖 Full messaging reference →](https://layer.ankitchaubey.in/messaging/sending.html)

<br/>

---

## 📎 Media

### Upload

```rust
use layer_client::media::UploadedFile;

// Upload from bytes — small files sequentially
let uploaded: UploadedFile = client
    .upload_file("photo.jpg", file_bytes.as_ref())
    .await?;

// Upload from bytes — parallel chunks (faster for large files)
let uploaded = client
    .upload_file_concurrent("video.mp4", video_bytes.as_ref())
    .await?;

// Upload from an async reader (e.g. a file on disk)
use tokio::fs::File;
let f = File::open("document.pdf").await?;
let uploaded = client
    .upload_stream("document.pdf", f)
    .await?;

// Send the uploaded file to a peer
client.send_file(peer.clone(), uploaded, /* as_photo */ false).await?;

// Send multiple files as an album in one call
client.send_album(peer.clone(), vec![uploaded_a, uploaded_b]).await?;
```

### Download

```rust
// Download directly to a file path (streaming, no full memory buffer)
client
    .download_media_to_file(&message_media, "output.jpg")
    .await?;

// Download to Vec<u8> — sequential
let bytes: Vec<u8> = client.download_media(&message_media).await?;

// Download to Vec<u8> — parallel chunks
let bytes: Vec<u8> = client.download_media_concurrent(&message_media).await?;

// Use the Downloadable trait for Photos, Documents, Stickers
use layer_client::media::{Photo, Downloadable};
let photo = Photo::from_message(&msg.raw)?;
let bytes = client.download(&photo).await?;
```

> [📖 Media guide →](https://layer.ankitchaubey.in/messaging/media.html)

<br/>

---

## ⌨️ Keyboards and Reply Markup

### Inline keyboards

```rust
use layer_client::keyboard::InlineKeyboard;

let kb = InlineKeyboard::new()
    .row()
        .callback("👍 Like",    b"like")
        .callback("👎 Dislike", b"dislike")
    .row()
        .url("🔗 Open docs", "https://docs.rs/layer-client")
        .switch_inline("🔍 Search", "query")
    .build();

client
    .send_message_to_peer_ex(peer.clone(), &InputMessage::text("Vote!").keyboard(kb))
    .await?;
```

Available button types: `callback`, `url`, `url_auth`, `switch_inline`, `switch_elsewhere`, `webview`, `simple_webview`, `request_phone`, `request_geo`, `request_poll`, `request_quiz`, `game`, `buy`, `copy_text`.

### Reply keyboards

```rust
use layer_client::keyboard::ReplyKeyboard;

let kb = ReplyKeyboard::new()
    .row()
        .text("📸 Photo")
        .text("📄 Document")
    .row()
        .text("❌ Cancel")
    .resize(true)
    .single_use(true)
    .build();

client
    .send_message_to_peer_ex(peer.clone(), &InputMessage::text("Choose:").keyboard(kb))
    .await?;
```

### Answer callback queries

```rust
Update::CallbackQuery(cb) => {
    let data = cb.data().unwrap_or("");
    match data {
        b"like"    => client.answer_callback_query(cb.query_id, Some("❤️ Liked!"), false).await?,
        b"dislike" => client.answer_callback_query(cb.query_id, Some("👎 Noted"),   false).await?,
        _          => client.answer_callback_query(cb.query_id, None,              false).await?,
    }
}
```

Pass `alert: true` as the third argument to show a popup alert instead of a toast.

### Inline mode

```rust
use layer_tl_types as tl;

Update::InlineQuery(iq) => {
    let q   = iq.query().to_string();
    let qid = iq.query_id;

    let results = vec![
        tl::enums::InputBotInlineResult::InputBotInlineResult(
            tl::types::InputBotInlineResult {
                id: "1".into(), r#type: "article".into(),
                title: Some("Result title".into()),
                description: Some(q.clone()),
                url: None, thumb: None, content: None,
                send_message: tl::enums::InputBotInlineMessage::Text(
                    tl::types::InputBotInlineMessageText {
                        no_webpage: false, invert_media: false,
                        message: q, entities: None, reply_markup: None,
                    },
                ),
            },
        ),
    ];

    // cache_time: 30s, is_personal: false, next_offset: None
    client.answer_inline_query(qid, results, 30, false, None).await?;
}
```

> [📖 Keyboards guide →](https://layer.ankitchaubey.in/messaging/keyboards.html)

<br/>

---

## 🖊️ Text Formatting

### Markdown

```rust
use layer_client::parsers::{parse_markdown, generate_markdown};

// Parse markdown → plain text + message entities
let (text, entities) = parse_markdown("**Bold**, `code`, _italic_, [link](https://example.com)");

// Send with formatting
client
    .send_message_to_peer_ex(
        peer.clone(),
        &InputMessage::text(text).entities(entities),
    )
    .await?;

// Go the other way: entities + plain text → markdown string
let md = generate_markdown(&plain_text, &entities);
```

### HTML

Enable the `html` or `html5ever` feature flag:

```toml
layer-client = { version = "0.4.6", features = ["html"] }
```

```rust
use layer_client::parsers::{parse_html, generate_html};

let (text, entities) = parse_html("<b>Bold</b> and <code>monospace</code>");

client
    .send_message_to_peer_ex(peer.clone(), &InputMessage::text(text).entities(entities))
    .await?;

// Always available, no feature flag needed
let html_str = generate_html(&plain_text, &entities);
```

> [📖 Formatting reference →](https://layer.ankitchaubey.in/messaging/formatting.html)

<br/>

---

## 💥 Reactions

[`InputReactions`](https://docs.rs/layer-client/latest/layer_client/reactions/struct.InputReactions.html) is the typed builder for reactions:

```rust
use layer_client::reactions::InputReactions;

// Single emoji reaction
client.send_reaction(peer.clone(), message_id, InputReactions::emoticon("👍")).await?;

// Custom premium emoji
client.send_reaction(peer.clone(), message_id, InputReactions::custom_emoji(1234567890)).await?;

// Big animated reaction
client.send_reaction(peer.clone(), message_id, InputReactions::emoticon("🔥").big()).await?;

// Remove all reactions
client.send_reaction(peer.clone(), message_id, InputReactions::remove()).await?;
```

> [📖 Reactions guide →](https://layer.ankitchaubey.in/messaging/reactions.html)

<br/>

---

## ⌛ Typing Guard (RAII)

[`TypingGuard`](https://docs.rs/layer-client/latest/layer_client/struct.TypingGuard.html) is a RAII wrapper that automatically starts and stops typing/uploading indicators:

```rust
use layer_client::TypingGuard;
use layer_tl_types as tl;

async fn handle_long_task(client: &Client, peer: tl::enums::Peer) -> anyhow::Result<()> {
    // Typing indicator starts immediately and is renewed every ~4 seconds
    let _typing = TypingGuard::start(
        client,
        peer.clone(),
        tl::enums::SendMessageAction::SendMessageTypingAction,
    )
    .await?;

    // Do expensive work — user sees "typing..."
    do_expensive_work().await;

    // _typing is dropped here → Telegram sees the indicator stop
    Ok(())
}
```

Convenience constructors for common actions:

```rust
// Typing
let _t = client.typing(peer.clone()).await?;

// Uploading document
let _t = client.uploading_document(peer.clone()).await?;

// Recording video
let _t = client.recording_video(peer.clone()).await?;

// Typing in a specific forum topic
let _t = client.typing_in_topic(peer.clone(), topic_id).await?;
```

> [📖 Typing guard reference →](https://layer.ankitchaubey.in/api/typing-guard.html)

<br/>

---

## 👥 Participants and Chat Management

### Fetch participants

```rust
use layer_client::participants::Participant;

// Fetch up to N participants at once
let participants: Vec<Participant> = client.get_participants(peer.clone(), 100).await?;

// Paginated lazy iterator — works for very large groups
let mut iter = client.iter_participants(peer.clone());
while let Some(p) = iter.next(&client).await? {
    println!("{}", p.user.first_name.as_deref().unwrap_or(""));
}

// Search within a group
let results = client.search_peer(peer.clone(), "John").await?;
```

### Ban, kick, promote

```rust
use layer_client::participants::{BanRights, AdminRightsBuilder};

// Kick (ban + immediate unban)
client.kick_participant(peer.clone(), user_id).await?;

// Ban with custom rights and optional expiry
client
    .ban_participant(
        peer.clone(),
        user_id,
        BanRights::new()
            .no_messages(true)
            .no_media(true)
            .until(expiry_unix_timestamp),
    )
    .await?;

// Promote to admin with specific rights
client
    .promote_participant(
        peer.clone(),
        user_id,
        AdminRightsBuilder::new()
            .post_messages(true)
            .delete_messages(true)
            .ban_users(true)
            .title("Moderator"),
    )
    .await?;

// Get a user's current permissions in a channel
let perms = client.get_permissions(peer.clone(), user_id).await?;
```

### Profile photos

```rust
// Fetch the first page of profile photos
let photos = client.get_profile_photos(user_id, 0, 10).await?;

// Lazy iterator across all pages
let mut iter = client.iter_profile_photos(user_id);
while let Some(photo) = iter.next(&client).await? {
    let bytes = client.download(&photo).await?;
}
```

### Join and leave

```rust
// Join a public group or channel by username
client.join_chat("@somegroup").await?;

// Accept a private invite link
client.accept_invite_link("https://t.me/joinchat/AbCdEfG...").await?;

// Leave and delete a dialog from the dialog list
client.delete_dialog(peer.clone()).await?;
```

> [📖 Participants guide →](https://layer.ankitchaubey.in/api/participants.html)

<br/>

---

## 🔍 Search

### In-chat search

[`SearchBuilder`](https://docs.rs/layer-client/latest/layer_client/search/struct.SearchBuilder.html) is a chainable builder for `messages.search`:

```rust
use layer_tl_types::enums::MessagesFilter;

let results = client
    .search(peer.clone(), "hello world")
    .min_date(1_700_000_000)
    .max_date(1_720_000_000)
    .filter(MessagesFilter::InputMessagesFilterPhotos)
    .limit(50)
    .fetch(&client)
    .await?;

for msg in results {
    println!("[{}] {}", msg.id, msg.message);
}
```

### Global search

[`GlobalSearchBuilder`](https://docs.rs/layer-client/latest/layer_client/search/struct.GlobalSearchBuilder.html) searches across all chats:

```rust
let results = client
    .search_global_builder("rust async")
    .broadcasts_only(true)       // channels only
    .min_date(1_700_000_000)
    .limit(30)
    .fetch(&client)
    .await?;
```

> [📖 Search guide →](https://layer.ankitchaubey.in/api/search.html)

<br/>

---

## 📜 Dialogs and Iterators

```rust
// Fetch the first N dialogs
let dialogs = client.get_dialogs(50).await?;
for d in &dialogs {
    println!("{} — {} unread", d.title(), d.unread_count());
}

// Lazy dialog iterator (all dialogs, paginated)
let mut iter = client.iter_dialogs();
while let Some(dialog) = iter.next(&client).await? {
    println!("{}", dialog.title());
}

// Lazy message iterator for a specific peer
let mut iter = client.iter_messages(peer.clone());
while let Some(msg) = iter.next(&client).await? {
    println!("{}", msg.message);
}

// Fetch messages by ID
let messages = client.get_messages_by_id(peer.clone(), &[100, 101, 102]).await?;

// Fetch the latest N messages from a peer
let messages = client.get_messages(peer.clone(), 20).await?;
```

> [📖 Dialogs guide →](https://layer.ankitchaubey.in/api/dialogs.html)

<br/>

---

## 🔗 Peer Resolution

```rust
// Resolve any string (username, phone number, "me") to a TL Peer
let peer = client.resolve_peer("@telegram").await?;
let peer = client.resolve_peer("+1234567890").await?;
let peer = client.resolve_peer("me").await?;

// Resolve just the username part (without @)
let peer = client.resolve_username("telegram").await?;
```

Access hash caching is handled automatically. Once a peer is resolved its access hash is stored in the session and reused on all subsequent calls — no need to manage it yourself.

<br/>

---

## 💾 Session Backends

layer ships with multiple session backends. They all implement the [`SessionBackend`](https://docs.rs/layer-client/latest/layer_client/session_backend/trait.SessionBackend.html) trait and are hot-swappable.

| Backend | Feature flag | Best for |
|---|---|---|
| [`BinaryFileBackend`](https://docs.rs/layer-client/latest/layer_client/session_backend/struct.BinaryFileBackend.html) | *(default)* | Single-process bots, scripts |
| [`InMemoryBackend`](https://docs.rs/layer-client/latest/layer_client/session_backend/struct.InMemoryBackend.html) | *(default)* | Tests, ephemeral tasks |
| [`StringSessionBackend`](https://docs.rs/layer-client/latest/layer_client/session_backend/struct.StringSessionBackend.html) | *(default)* | Serverless, env-var storage, CI bots |
| [`SqliteBackend`](https://docs.rs/layer-client/latest/layer_client/session_backend/struct.SqliteBackend.html) | `sqlite-session` | Multi-session local apps |
| [`LibSqlBackend`](https://docs.rs/layer-client/latest/layer_client/session_backend/struct.LibSqlBackend.html) | `libsql-session` | Distributed / Turso-backed storage |
| Custom | — | Implement `SessionBackend` for anything |

```rust
use layer_client::session_backend::{SqliteBackend, SessionBackend};

// SQLite backend
let backend = SqliteBackend::new("sessions.db").await?;

let (client, _shutdown) = Client::connect(Config {
    session_backend: Box::new(backend),
    api_id:  12345,
    api_hash: "your_api_hash".into(),
    ..Default::default()
}).await?;
```

```rust
// Implement your own — Redis, Postgres, S3, anything
use layer_client::session_backend::SessionBackend;

struct RedisBackend { /* ... */ }

#[async_trait::async_trait]
impl SessionBackend for RedisBackend {
    async fn load(&self) -> anyhow::Result<Option<Vec<u8>>> { /* ... */ }
    async fn save(&self, data: &[u8]) -> anyhow::Result<()> { /* ... */ }
}
```

> [📖 Session backends guide →](https://layer.ankitchaubey.in/authentication/session-backends.html)

<br/>

---

## 🔧 Feature Flags

### `layer-tl-types`

| Flag | Default | Description |
|---|:---:|---|
| `tl-api` | ✅ | High-level Telegram API schema (`api.tl`) |
| `tl-mtproto` | ❌ | Low-level MTProto schema (`mtproto.tl`) |
| `impl-debug` | ✅ | `#[derive(Debug)]` on all generated types |
| `impl-from-type` | ✅ | `From<types::T> for enums::E` on all constructors |
| `impl-from-enum` | ✅ | `TryFrom<enums::E> for types::T` on all constructors |
| `name-for-id` | ❌ | `name_for_id(u32) -> Option<&'static str>` lookup table |
| `impl-serde` | ❌ | `serde::Serialize` + `Deserialize` on all types |

### `layer-client`

| Flag | Default | Description |
|---|:---:|---|
| `html` | ❌ | Hand-rolled HTML parser (`parse_html`, `generate_html`) |
| `html5ever` | ❌ | Spec-compliant `html5ever` tokenizer, replaces the built-in parser |
| `sqlite-session` | ❌ | SQLite session backend (`SqliteBackend`) |
| `libsql-session` | ❌ | libsql / Turso session backend (`LibSqlBackend`) |

<br/>

---

## 🔩 Raw API Escape Hatch

Every Telegram method in **Layer 224** is available via the raw [`invoke`](https://docs.rs/layer-client/latest/layer_client/struct.Client.html#method.invoke) API, even if it has no high-level wrapper yet. The full type-safe schema is available as `layer_client::tl` (re-exported from `layer-tl-types`).

```rust
use layer_client::tl;

// Set the bot's command list — no wrapper yet, use raw invoke
let req = tl::functions::bots::SetBotCommands {
    scope: tl::enums::BotCommandScope::Default(tl::types::BotCommandScopeDefault {}),
    lang_code: "en".into(),
    commands: vec![
        tl::enums::BotCommand::BotCommand(tl::types::BotCommand {
            command:     "start".into(),
            description: "Start the bot".into(),
        }),
    ],
};
client.invoke(&req).await?;
```

```rust
// Update profile info
let req = tl::functions::account::UpdateProfile {
    first_name: Some("Alice".into()),
    last_name:  None,
    about:      Some("layer user 🦀".into()),
};
client.invoke(&req).await?;
```

```rust
// Send to a specific DC (useful for cross-DC file downloads)
client.invoke_on_dc(&req, 2).await?;
```

Any method listed in the [Telegram API documentation](https://core.telegram.org/method) can be invoked this way. Layer 224 includes **2,329** TL constructors and all RPC functions.

> [📖 Raw API guide →](https://layer.ankitchaubey.in/advanced/raw-api.html)

<br/>

---

## 🚂 Transports

Three MTProto transport encodings are supported:

| Transport | Description | When to use |
|---|---|---|
| **Abridged** | Single-byte length prefix, lowest overhead | Default — best for most setups |
| **Intermediate** | 4-byte LE length prefix | Better compatibility with some proxies |
| **Obfuscated2** | XOR stream cipher over Abridged | DPI bypass, MTProxy, restricted networks |

```rust
use layer_client::{Client, TransportKind};

// Switch to Obfuscated2 (DPI bypass)
let (client, _) = Client::builder()
    .api_id(12345)
    .api_hash("your_api_hash")
    .session("obfuscated.session")
    .transport(TransportKind::Obfuscated)
    .connect()
    .await?;
```

> [📖 Transport reference →](https://layer.ankitchaubey.in/advanced/proxy.html)

<br/>

---

## 🌐 Networking — SOCKS5 and DC Pool

### SOCKS5 proxy

```rust
use layer_client::{Client, Socks5Config};

// Without auth
let (client, _) = Client::builder()
    .api_id(12345)
    .api_hash("your_api_hash")
    .session("proxy.session")
    .socks5("127.0.0.1", 1080)
    .connect()
    .await?;

// With username/password
let proxy = Socks5Config::with_auth("proxy.host", 1080, "user", "pass");
let (client, _) = Client::builder()
    .api_id(12345)
    .api_hash("your_api_hash")
    .socks5_config(proxy)
    .connect()
    .await?;
```

### DC pool and multi-DC

Auth keys are stored per datacenter and connections are created on demand. When Telegram responds with `PHONE_MIGRATE_*`, `USER_MIGRATE_*`, or `NETWORK_MIGRATE_*`, the client migrates automatically. You can also target a specific DC directly:

```rust
// Force a request to DC 2
client.invoke_on_dc(&req, 2).await?;
```

### Reconnect and keepalive

The client reconnects automatically after network failures using exponential backoff with 20% jitter, capped at 5 seconds (tuned for mobile / Android conditions). Pings are sent every 60 seconds. To skip the backoff after a known-good network event:

```rust
// Call this when your app detects the network is back
client.signal_network_restored();
```

<br/>

---

## ⚠️ Error Handling

```rust
use layer_client::{InvocationError, RpcError};

match client.send_message("@badpeer", "Hello").await {
    Ok(()) => {}

    // Telegram RPC error — has a numeric code and a string message
    Err(InvocationError::Rpc(RpcError { code, message, .. })) => {
        eprintln!("Telegram error {code}: {message}");
    }

    // Network / I/O error
    Err(InvocationError::Io(e)) => {
        eprintln!("I/O error: {e}");
    }

    // Other
    Err(e) => eprintln!("Error: {e}"),
}
```

`FLOOD_WAIT` errors are handled automatically by the default [`AutoSleep`](https://docs.rs/layer-client/latest/layer_client/retry/struct.AutoSleep.html) retry policy. You can replace this with your own policy:

```rust
use layer_client::retry::NoRetries;

// Disable all automatic retries
let (client, _) = Client::builder()
    .api_id(12345)
    .api_hash("your_api_hash")
    .retry_policy(NoRetries)
    .connect()
    .await?;
```

> [📖 Error handling guide →](https://layer.ankitchaubey.in/errors.html)

<br/>

---

## 🛑 Shutdown

```rust
// Client::connect returns (Client, ShutdownToken)
let (client, shutdown) = Client::connect(config).await?;

// Graceful shutdown from any task
shutdown.cancel();

// Immediate disconnect (no drain)
client.disconnect();
```

The [`ShutdownToken`](https://docs.rs/layer-client/latest/layer_client/struct.ShutdownToken.html) is a `CancellationToken` wrapper. You can clone it and pass it to multiple tasks.

<br/>

---

## 📐 Updating the TL Layer

When Telegram publishes a new TL schema, updating layer is a two-step process:

```bash
# 1. Replace the schema file
cp new-api.tl layer-tl-types/tl/api.tl

# 2. Build — layer-tl-gen regenerates all types at compile time
cargo build
```

The codegen (`layer-tl-gen`) runs as a build script. No manual code changes are required for pure schema updates — the 2,329 type definitions are entirely auto-generated.

> [📖 Layer upgrade guide →](https://layer.ankitchaubey.in/advanced/layer-upgrade.html)

<br/>

---

## 🧪 Running Tests

```bash
# Run all tests in the workspace
cargo test --workspace

# Run only layer-client tests
cargo test -p layer-client

# Run with all features enabled
cargo test --workspace --all-features
```

Integration tests live in [`layer-client/tests/integration.rs`](./layer-client/tests/integration.rs). They use `InMemoryBackend` and do not require real Telegram credentials.

<br/>

---

## ❌ Unsupported Features

The following are gaps in the current high-level API. Every single one can be accessed today via `client.invoke::<R>()` with the raw TL types — see the [Raw API Escape Hatch](#-raw-api-escape-hatch) section.

| Feature | Workaround |
|---|---|
| **Secret chats (E2E)** | Not implemented at the MTProto layer-2 level |
| **Voice and video calls** | No call signalling or media transport |
| **Payments** | `SentCode::PaymentRequired` returns an error |
| **Channel creation** | Use `invoke` with `channels::CreateChannel` |
| **Sticker set management** | Use `invoke` with `messages::GetStickerSet` etc. |
| **Account settings** | Use `invoke` with `account::UpdateProfile` etc. |
| **Contact management** | Use `invoke` with `contacts::ImportContacts` etc. |
| **Poll / quiz creation** | Use `invoke` with `InputMediaPoll` |
| **Live location** | Not wrapped |
| **Bot command registration** | Use `invoke` with `bots::SetBotCommands` |
| **IPv6** | Config flag exists but address formatting for IPv6 DCs is untested |

<br/>

---

## 💬 Community

Questions, ideas, bug reports — come talk to us:

| | Link |
|---|---|
| 📢 **Channel** — releases and announcements | [t.me/layer_rs](https://t.me/layer_rs) |
| 💬 **Chat** — questions and discussion | [t.me/layer_chat](https://t.me/layer_chat) |
| 📖 **Online Book** — narrative guide | [layer.ankitchaubey.in](https://layer.ankitchaubey.in/) |
| 📦 **Crates.io** | [crates.io/crates/layer-client](https://crates.io/crates/layer-client) |
| 📄 **API Docs** | [docs.rs/layer-client](https://docs.rs/layer-client) |
| 🐛 **Issue Tracker** | [github.com/ankit-chaubey/layer/issues](https://github.com/ankit-chaubey/layer/issues) |

<br/>

---

## 🤝 Contributing

Contributions are welcome — bug fixes, new wrappers, better docs, more tests. All pull requests are appreciated.

Please read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a PR. In brief:

- Run `cargo test --workspace` and `cargo clippy --workspace` locally before pushing.
- For new wrappers, add a doc-test in the `///` comment block.
- For security issues, follow the responsible disclosure process in [SECURITY.md](SECURITY.md) — **do not** open a public issue.

[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen?style=flat-square)](CONTRIBUTING.md)
[![Good First Issues](https://img.shields.io/github/issues/ankit-chaubey/layer/good%20first%20issue?style=flat-square&color=5865F2&label=good%20first%20issues)](https://github.com/ankit-chaubey/layer/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22)

<br/>

---

## 🔒 Security

Found a vulnerability? Please report it **privately**. See [SECURITY.md](SECURITY.md) for the responsible disclosure process. Do not open a public GitHub issue for security bugs.

<br/>

---

## 👤 Author

<div align="center">

<br/>

<a href="https://github.com/ankit-chaubey">
  <img src="https://github.com/ankit-chaubey.png" width="96" style="border-radius:50%" alt="Ankit Chaubey" />
</a>

<br/><br/>

**Ankit Chaubey**

*Built with curiosity, caffeine, and a lot of Rust compiler errors 🦀*

<br/>

[![GitHub](https://img.shields.io/badge/GitHub-ankit--chaubey-181717?style=for-the-badge&logo=github)](https://github.com/ankit-chaubey)
[![Website](https://img.shields.io/badge/Website-ankitchaubey.in-10b981?style=for-the-badge&logo=safari)](https://ankitchaubey.in)
[![Email](https://img.shields.io/badge/Email-ankitchaubey.dev%40gmail.com-ea4335?style=for-the-badge&logo=gmail)](mailto:ankitchaubey.dev@gmail.com)
[![Telegram](https://img.shields.io/badge/Telegram-%40layer__rs-2CA5E0?style=for-the-badge&logo=telegram)](https://t.me/layer_rs)

<br/>

</div>

---

## 🙏 Acknowledgements

- [**Lonami**](https://codeberg.org/Lonami) for [**grammers**](https://codeberg.org/Lonami/grammers) — the architecture, DH session design, SRP 2FA math, and session handling in layer are deeply inspired by this excellent library. Portions of this project include code derived from grammers, which is dual-licensed MIT or Apache-2.0.

- [**Telegram**](https://core.telegram.org/mtproto) for the detailed MTProto specification and the publicly available TL schema.

- The Rust async ecosystem — [`tokio`](https://tokio.rs), [`flate2`](https://crates.io/crates/flate2), [`getrandom`](https://crates.io/crates/getrandom), [`sha2`](https://crates.io/crates/sha2), [`socket2`](https://crates.io/crates/socket2), and friends.

<br/>

---

## 📄 License

Licensed under either of, at your option:

- **MIT License** — see [LICENSE-MIT](LICENSE-MIT)
- **Apache License, Version 2.0** — see [LICENSE-APACHE](LICENSE-APACHE)

Unless you explicitly state otherwise, any contribution you submit for inclusion shall be dual-licensed as above, without any additional terms or conditions.

<br/>

---

## ⚠️ Telegram Terms of Service

As with any third-party Telegram library, ensure your usage complies with [Telegram's Terms of Service](https://core.telegram.org/api/terms) and [API Terms of Service](https://core.telegram.org/api/terms). Misuse of the Telegram API — including but not limited to spam, mass scraping, or automation of normal user accounts — may result in account limitations or permanent bans.

<br/>

---

<div align="center">

*layer — because sometimes you have to build it yourself to truly understand it.*

[![Star on GitHub](https://img.shields.io/github/stars/ankit-chaubey/layer?style=social)](https://github.com/ankit-chaubey/layer)

</div>
