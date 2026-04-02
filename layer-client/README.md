<div align="center">

<img src="https://raw.githubusercontent.com/ankit-chaubey/layer/main/docs/images/crate-client-banner.svg" alt="layer-client" width="100%" />

# 🤝 layer-client

**High-level async Telegram client for Rust.**

[![Crates.io](https://img.shields.io/crates/v/layer-client?color=fc8d62)](https://crates.io/crates/layer-client)
[![docs.rs](https://img.shields.io/badge/docs.rs-layer--client-5865F2)](https://docs.rs/layer-client)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-2024_edition-f74c00)](https://www.rust-lang.org/)
[![TL Layer](https://img.shields.io/badge/TL%20Layer-224-8b5cf6)](https://core.telegram.org/schema)

*Connect, authenticate, send messages, and stream updates with a clean async API.*

</div>

---

## Table of Contents

- [Installation](#-installation)
- [What It Does](#-what-it-does)
- [Minimal Bot — 15 Lines](#-minimal-bot--15-lines)
- [Connecting — ClientBuilder](#-connecting--clientbuilder)
- [Authentication](#-authentication)
- [String Sessions — Portable Auth](#-string-sessions--portable-auth)
- [Update Stream](#-update-stream)
- [Messaging](#-messaging)
- [InputMessage Builder](#-inputmessage-builder)
- [Keyboards](#-keyboards)
- [Media Upload & Download](#-media-upload--download)
- [Text Formatting](#-text-formatting)
- [Reactions](#-reactions)
- [Typing Guard — RAII](#-typing-guard--raii)
- [Participants & Chat Management](#-participants--chat-management)
- [Search](#-search)
- [Dialogs & Iterators](#-dialogs--iterators)
- [Peer Resolution](#-peer-resolution)
- [Session Backends](#-session-backends)
- [Transport & Networking](#-transport--networking)
- [Feature Flags](#-feature-flags)
- [Configuration Reference](#-configuration-reference)
- [Error Handling](#-error-handling)
- [Raw API Escape Hatch](#-raw-api-escape-hatch)
- [Shutdown](#-shutdown)

---

## 📦 Installation

```toml
[dependencies]
layer-client = "0.4.5"
tokio = { version = "1", features = ["full"] }
```

---

## ✨ What It Does

`layer-client` wraps the raw MTProto machinery into a clean, ergonomic async API. You don't need to know anything about TL schemas, DH handshakes, or message framing — just connect and go.

- 🔐 **User auth** — phone code + optional 2FA (SRP)
- 🤖 **Bot auth** — bot token login
- 💬 **Messaging** — send, edit, delete, forward, pin, schedule
- 📎 **Media** — upload files with automatic chunking, download with streaming
- 📡 **Update stream** — typed async events (`NewMessage`, `CallbackQuery`, `InlineQuery`, etc.)
- ⌨️ **Keyboards** — fluent inline and reply keyboard builders
- 🔁 **FLOOD_WAIT retries** — automatic with configurable policy
- 🌐 **DC migration** — handled transparently
- 💾 **Session persistence** — five backends (file, memory, string, SQLite, libSQL)
- 🧦 **SOCKS5 proxy** — route all connections through a proxy
- 🔧 **Raw API access** — `client.invoke(req)` for any TL function

---

## 🚀 Minimal Bot — 15 Lines

```rust
use layer_client::{Client, update::Update};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (client, _sd) = Client::builder()
        .api_id(12345)
        .api_hash("abc123")
        .session("bot.session")
        .connect().await?;

    client.bot_sign_in("1234567890:ABCdef...").await?;

    let mut updates = client.stream_updates();
    while let Some(Update::NewMessage(msg)) = updates.next().await {
        if let Some(text) = msg.text() {
            msg.reply(&client, &format!("Echo: {text}")).await?;
        }
    }
    Ok(())
}
```

---

## 🔌 Connecting — ClientBuilder

The preferred way to connect is through the fluent `ClientBuilder`:

```rust
use layer_client::Client;

let (client, shutdown) = Client::builder()
    .api_id(12345)                      // from https://my.telegram.org
    .api_hash("abc123")
    .session("my.session")              // file-backed session
    .catch_up(true)                     // replay missed updates on reconnect
    .connect()
    .await?;
```

Other session options:

```rust
// In-memory session (lost on restart)
.in_memory()

// Portable base64 string session (for cloud deployments)
.session_string(std::env::var("SESSION").unwrap_or_default())
```

You can also use the lower-level `Client::connect(Config { ... })` API directly if you prefer explicit struct construction.

---

## 🔐 Authentication

### User Login

```rust
use layer_client::SignInError;

if !client.is_authorized().await? {
    let token = client.request_login_code("+1234567890").await?;

    // prompt user for the code they received
    let code = read_line("Enter code: ");

    match client.sign_in(&token, &code).await {
        Ok(name) => println!("✅ Signed in as {name}"),
        Err(SignInError::PasswordRequired(pw_token)) => {
            let hint = pw_token.hint().unwrap_or("(no hint)");
            println!("2FA required — hint: {hint}");
            let password = read_line("Enter password: ");
            client.check_password(pw_token, &password).await?;
        }
        Err(e) => return Err(e.into()),
    }

    client.save_session().await?;
}
```

### Bot Login

```rust
client.bot_sign_in("1234567890:ABCdef...").await?;
client.save_session().await?;
```

### Other Methods

```rust
// Get the currently logged-in user
let me = client.get_me().await?;
println!("Logged in as: {} (id={})", me.first_name.unwrap_or_default(), me.id);

// Sign out (invalidates the server-side session)
client.sign_out().await?;
```

---

## 📦 String Sessions — Portable Auth

A string session encodes your auth key and DC info as a base64 string — useful for deploying bots to servers or cloud functions without shipping a session file.

```rust
// Export the session to a string
let session_str = client.export_session_string().await?;
println!("SESSION={session_str}");

// Later, restore it:
let (client, _sd) = Client::builder()
    .api_id(12345)
    .api_hash("abc123")
    .session_string(std::env::var("SESSION").unwrap())
    .connect().await?;
```

---

## 📡 Update Stream

```rust
let mut stream = client.stream_updates();

while let Some(update) = stream.next().await {
    match update {
        Update::NewMessage(msg)      => { /* new incoming message */ }
        Update::MessageEdited(msg)   => { /* message was edited */   }
        Update::MessageDeleted(del)  => { /* messages deleted */     }
        Update::CallbackQuery(cb)    => { /* inline button pressed */ }
        Update::InlineQuery(iq)      => { /* @bot inline query */     }
        Update::InlineSend(is)       => { /* inline result chosen */  }
        Update::ChatAction(ca)       => { /* user joined/left/etc */  }
        Update::UserStatus(us)       => { /* online/offline status */ }
        Update::Raw(raw)             => { /* unhandled constructor */ }
        _ => {}
    }
}
```

### IncomingMessage API

```rust
msg.id()               // → i32
msg.text()             // → Option<&str>
msg.markdown_text()    // → Option<String>   (text + entities as Markdown)
msg.html_text()        // → Option<String>   (text + entities as HTML)
msg.date()             // → i32              (Unix timestamp)
msg.peer_id()          // → Option<&Peer>    (the chat this belongs to)
msg.sender_id()        // → Option<&Peer>    (None for anonymous posts)
msg.outgoing()         // → bool
msg.via_bot_id()       // → Option<i64>
msg.reply_to_msg_id()  // → Option<i32>

// Reply with text
msg.reply(&client, "text").await?;

// Reply with an InputMessage builder
msg.reply_ex(&client, InputMessage::text("bold").entities(vec![...]))await?;
```

### Spawning per-update tasks

```rust
use std::sync::Arc;
let client = Arc::new(client);

while let Some(Update::NewMessage(msg)) = stream.next().await {
    let c = Arc::clone(&client);
    tokio::spawn(async move {
        if let Err(e) = handle(&c, msg).await {
            eprintln!("handler error: {e}");
        }
    });
}
```

---

## 💬 Messaging

```rust
// Send by username, phone, "me", or numeric ID
client.send_message("@username", "Hello!").await?;
client.send_message("me", "Saved message!").await?;
client.send_to_self("Quick note").await?;

// Send to a resolved peer directly
client.send_message_to_peer(peer.clone(), "text").await?;

// Send with full InputMessage options (see next section)
client.send_message_to_peer_ex(peer, input_msg).await?;

// Edit a message
client.edit_message(peer, msg_id, "new text").await?;

// Edit an inline message (for inline bot results)
client.edit_inline_message(inline_msg_id, new_input_message).await?;

// Forward messages
client.forward_messages(from_peer, vec![msg_id1, msg_id2], to_peer).await?;

// Delete messages
client.delete_messages(peer, vec![123, 456], /*revoke=*/true).await?;

// Pin / unpin
client.pin_message(peer, msg_id, /*notify=*/false, /*both_sides=*/false).await?;
client.unpin_message(peer, msg_id).await?;
client.unpin_all_messages(peer).await?;

// Fetch message history
let msgs = client.get_messages(peer, /*limit=*/50, /*offset_id=*/0).await?;

// Fetch specific messages by ID
let msgs = client.get_messages_by_id(peer, &[101, 202, 303]).await?;

// Fetch pinned message
let pinned = client.get_pinned_message(peer).await?;

// Scheduled messages
let scheduled = client.get_scheduled_messages(peer).await?;
client.delete_scheduled_messages(peer, &[msg_id]).await?;

// Mark as read
client.mark_as_read(peer).await?;

// Clear unread mentions
client.clear_mentions(peer).await?;
```

---

## 📝 InputMessage Builder

`InputMessage` is the fluent builder for composing rich outgoing messages:

```rust
use layer_client::InputMessage;

let msg = InputMessage::text("Hello **world**!")
    .reply_to(Some(orig_msg_id))
    .silent(true)
    .no_webpage(true)
    .schedule_date(Some(unix_ts))          // schedule for later
    .schedule_once_online()                // send when recipient comes online
    .keyboard(inline_kb.into_reply_markup())
    .entities(parsed_entities)
    .copy_media(media_input);              // attach media from another message

client.send_message_to_peer_ex(peer, msg).await?;
```

---

## ⌨️ Keyboards

### Inline Keyboards

```rust
use layer_client::keyboard::{InlineKeyboard, Button};

let kb = InlineKeyboard::new()
    .row([
        Button::callback("✅ Yes", b"yes"),
        Button::callback("❌ No",  b"no"),
    ])
    .row([Button::url("📖 Docs", "https://docs.rs/layer-client")]);

let msg = InputMessage::text("Choose:").keyboard(kb);
```

### Reply Keyboards

```rust
use layer_client::keyboard::ReplyKeyboard;

let kb = ReplyKeyboard::new()
    .row(["Option A", "Option B"])
    .row(["Option C"]);

let msg = InputMessage::text("Pick one:").keyboard(kb);
```

### Answering Callback Queries

```rust
// Simple toast notification
client.answer_callback_query(cb.query_id, Some("Done!"), /*alert=*/false).await?;

// Alert popup
client.answer_callback_query(cb.query_id, Some("Warning!"), /*alert=*/true).await?;
```

### Answering Inline Queries

```rust
use layer_tl_types as tl;

let results = vec![
    tl::enums::InputBotInlineResult::Result(tl::types::InputBotInlineResult { ... }),
];

client.answer_inline_query(
    iq.query_id,
    results,
    /*cache_time=*/ 300,
    /*is_personal=*/ false,
    /*next_offset=*/ None,
).await?;
```

---

## 📎 Media Upload & Download

### Upload

```rust
// Upload a file from disk (auto-chooses sequential or concurrent based on size)
let uploaded = client.upload_file("photo.jpg").await?;

// Concurrent upload (parallel worker pool — faster for large files)
let uploaded = client.upload_file_concurrent("video.mp4", /*workers=*/4).await?;

// Upload from an AsyncRead stream
let uploaded = client.upload_stream(&mut reader, "name.bin", "application/octet-stream").await?;

// Send as photo
let media = uploaded.as_photo_media();
let msg = InputMessage::text("Caption").copy_media(media);
client.send_message_to_peer_ex(peer, msg).await?;

// Send as document (forces download button in Telegram)
let media = uploaded.as_document_media();
```

### Albums (Multi-media)

```rust
use layer_client::media::AlbumItem;

let items = vec![
    AlbumItem::new(photo1_media).caption("First"),
    AlbumItem::new(photo2_media).caption("Second"),
];
client.send_album(peer, items).await?;
```

### Download

```rust
// Download to a file
client.download_media_to_file(&msg_media, "output.jpg").await?;

// Download to bytes in memory
let bytes = client.download_media(&msg_media).await?;

// Concurrent download (multi-worker, faster for large files)
let bytes = client.download_media_concurrent(&msg_media, /*workers=*/4).await?;

// Streaming iterator (process chunks as they arrive)
let mut iter = client.iter_download(&msg_media);
while let Some(chunk) = iter.next(&client).await? {
    file.write_all(&chunk).await?;
}
```

### Typed Media Wrappers

```rust
use layer_client::media::{Photo, Document, Sticker, Downloadable};

// Extract from a message
if let Some(photo) = Photo::from_media(msg.raw_media()) {
    println!("photo id={}", photo.id());
    let bytes = client.download_media(&photo).await?;
}

if let Some(doc) = Document::from_media(msg.raw_media()) {
    println!("file: {:?}, {} bytes", doc.file_name(), doc.size());
    let bytes = client.download_media(&doc).await?;
}

if let Some(sticker) = Sticker::from_document(doc) {
    println!("emoji: {:?}", sticker.emoji());
}
```

---

## ✏️ Text Formatting

### Markdown

```rust
use layer_client::parsers::parse_markdown;

let (text, entities) = parse_markdown("Hello **world** and `code`!")?;
let msg = InputMessage::text(text).entities(entities);
```

### HTML

```rust
// Built-in HTML parser (always available, no feature flag)
use layer_client::parsers::parse_html;

let (text, entities) = parse_html("<b>Bold</b> and <code>code</code>")?;
```

With the `html5ever` feature, HTML parsing is backed by the spec-compliant `html5ever` tokenizer (handles malformed markup):

```toml
layer-client = { version = "0.4.5", features = ["html5ever"] }
```

### Formatting of incoming messages

```rust
// Reconstruct the Markdown from a received message's entities
let md   = msg.markdown_text();  // e.g. Some("Hello **world**!")
let html = msg.html_text();      // e.g. Some("Hello <b>world</b>!")
```

---

## 💜 Reactions

```rust
// Send a reaction
client.send_reaction(peer, msg_id, "👍").await?;

// Remove a reaction (empty string)
client.send_reaction(peer, msg_id, "").await?;
```

---

## ⌛ Typing Guard — RAII

`TypingGuard` automatically manages the typing indicator — sends it immediately, keeps it alive by re-sending every ~4 seconds, and cancels it on drop.

```rust
use layer_client::TypingGuard;
use layer_tl_types as tl;

async fn handle(client: &Client, peer: tl::enums::Peer) -> anyhow::Result<()> {
    // Typing indicator is live while `_guard` is in scope
    let _guard = TypingGuard::start(
        client,
        peer.clone(),
        tl::enums::SendMessageAction::SendMessageTypingAction,
    ).await?;

    let result = do_expensive_work().await;  // indicator stays alive here

    // `_guard` drops here → indicator is cancelled immediately
    Ok(result)
}
```

For explicit chat actions without RAII:

```rust
use layer_tl_types::enums::SendMessageAction;

client.send_chat_action(peer, SendMessageAction::SendMessageUploadDocumentAction(
    tl::types::SendMessageUploadDocumentAction { progress: 0 }
)).await?;
```

---

## 👥 Participants & Chat Management

```rust
// Fetch participants (paginated internally)
let participants = client.get_participants(peer, /*limit=*/100).await?;

// Search participants by name
let matches = client.search_peer("alice").await?;

// Kick (remove from group)
client.kick_participant(peer, user_peer).await?;

// Ban with granular rights
use layer_client::BanRights;
let rights = BanRights::default()
    .send_messages(false)
    .send_media(false);
client.ban_participant(peer, user_peer, rights).await?;

// Promote to admin
use layer_client::AdminRightsBuilder;
client.promote_participant(
    peer,
    user_peer,
    AdminRightsBuilder::new()
        .can_post_messages(true)
        .can_delete_messages(true),
).await?;

// Profile photos
let photos = client.get_profile_photos(user_peer).await?;

// Effective permissions
let perms = client.get_permissions(peer, user_peer).await?;

// Join a chat / channel
client.join_chat(peer).await?;

// Accept an invite link
client.accept_invite_link("https://t.me/+XXXXXXXX").await?;
```

---

## 🔍 Search

### In-chat search

```rust
let results = client
    .search(peer, "hello world")
    .min_date(1_700_000_000)
    .max_date(1_720_000_000)
    .filter(tl::enums::MessagesFilter::InputMessagesFilterPhotos)
    .limit(50)
    .fetch(&client)
    .await?;
```

### Global search

```rust
let results = client
    .search_global_builder("rust async")
    .broadcasts_only(true)
    .min_date(1_700_000_000)
    .limit(30)
    .fetch(&client)
    .await?;
```

---

## 📋 Dialogs & Iterators

```rust
// Fetch the first N dialogs (snapshot)
let dialogs = client.get_dialogs(100).await?;

// Streaming dialog iterator (all dialogs, paginated automatically)
let mut iter = client.iter_dialogs();
while let Some(dialog) = iter.next(&client).await? {
    println!("{}: {} unread", dialog.title(), dialog.unread_count());
}

// Streaming message iterator for a chat
let mut iter = client.iter_messages(peer);
iter.limit(200);
while let Some(msg) = iter.next(&client).await? {
    println!("[{}] {}", msg.id(), msg.text().unwrap_or("(media)"));
}

// Delete a dialog
client.delete_dialog(peer).await?;
```

---

## 🔎 Peer Resolution

```rust
// From a username
let peer = client.resolve_peer("@username").await?;

// From "me" (yourself)
let peer = client.resolve_peer("me").await?;

// From a numeric ID string
let peer = client.resolve_peer("123456789").await?;

// Resolve a username directly (returns the full User or Chat)
let entity = client.resolve_username("username").await?;

// Resolve to InputPeer (needed for raw API calls)
let input_peer = client.resolve_to_input_peer(peer).await?;
```

---

## 💾 Session Backends

| Backend | Type | Notes |
|---|---|---|
| `BinaryFileBackend` | File | Default — fast binary format, survives restarts |
| `InMemoryBackend` | Memory | Ephemeral — lost on restart; useful for tests |
| `StringSessionBackend` | String | Portable base64; inject via env var |
| `SqliteBackend` | SQLite | Requires `sqlite-session` feature |
| `LibSqlBackend` | libSQL | Requires `libsql-session` feature; works with Turso |

```rust
use layer_client::session_backend::{SqliteBackend, InMemoryBackend, StringSessionBackend};
use std::sync::Arc;

// SQLite
let (client, _sd) = Client::builder()
    .api_id(12345)
    .api_hash("abc123")
    .session_backend(Arc::new(SqliteBackend::open("sessions.db").await?))
    .connect().await?;

// In-memory (for tests)
Client::builder()
    .api_id(12345)
    .api_hash("abc123")
    .in_memory()
    .connect().await?;
```

---

## 🌐 Transport & Networking

### Transport kinds

```rust
use layer_client::TransportKind;

Client::builder()
    .transport(TransportKind::Abridged)      // default — lowest overhead
    .transport(TransportKind::Intermediate)   // fixed-width framing
    .transport(TransportKind::Obfuscated)     // XOR-encrypted, resists DPI
```

### SOCKS5 Proxy

```rust
use layer_client::Socks5Config;

Client::builder()
    .api_id(12345)
    .api_hash("abc123")
    .socks5(Socks5Config {
        proxy_addr: "127.0.0.1:1080".into(),
        auth: Some(("user".into(), "pass".into())),
    })
    .connect().await?;
```

### DC Pool

`layer-client` maintains a connection pool across Telegram's DCs. When an API call is routed to a different DC (e.g. for media downloads), the pool opens a new connection and caches it. DC migration after a `*_MIGRATE` error is handled transparently — no user code needed.

---

## 🚩 Feature Flags

| Feature | Default | Description |
|---|---|---|
| `sqlite-session` | ❌ | Enables `SqliteBackend` via `rusqlite` |
| `libsql-session` | ❌ | Enables `LibSqlBackend` via `libsql` (Turso-compatible) |
| `html` | ❌ | Built-in HTML ↔ entity parser |
| `html5ever` | ❌ | HTML parser backed by `html5ever` (spec-compliant; implies `html`) |
| `serde` | ❌ | Adds `serde::Serialize` / `Deserialize` on public types |

```toml
layer-client = { version = "0.4.5", features = ["sqlite-session", "html"] }
```

---

## ⚙️ Configuration Reference

```rust
Config {
    session_path:  "my.session".into(),          // file path for BinaryFileBackend
    api_id:        12345,                         // from https://my.telegram.org
    api_hash:      "abc123".into(),               // from https://my.telegram.org
    dc_addr:       None,                          // override initial DC (default: DC2)
    retry_policy:  Arc::new(AutoSleep::default()), // FLOOD_WAIT handler
    socks5:        None,                          // SOCKS5 proxy
    allow_ipv6:    false,                         // prefer IPv4 by default
    transport:     TransportKind::Abridged,       // wire framing format
    catch_up:      false,                         // replay missed updates on start
}
```

### Retry Policies

```rust
// Auto-sleep on FLOOD_WAIT (default) — sleeps for the duration Telegram specifies
retry_policy: Arc::new(AutoSleep::default())

// Never retry — propagate all errors immediately
retry_policy: Arc::new(NoRetries)

// Custom policy
struct MyPolicy;
impl RetryPolicy for MyPolicy {
    fn should_retry(&self, ctx: &RetryContext) -> ControlFlow<(), Duration> {
        if ctx.flood_wait_seconds < 60 {
            ControlFlow::Continue(Duration::from_secs(ctx.flood_wait_seconds as u64))
        } else {
            ControlFlow::Break(())  // give up for long waits
        }
    }
}
```

---

## ⚠️ Error Handling

```rust
use layer_client::{InvocationError, RpcError};

match client.send_message("@user", "hi").await {
    Ok(()) => {}
    Err(InvocationError::Rpc(RpcError { code, name, .. })) => {
        eprintln!("Telegram error {code}: {name}");
    }
    Err(e) => eprintln!("Transport/IO error: {e}"),
}
```

Common RPC errors: `FLOOD_WAIT_X`, `USER_DEACTIVATED`, `AUTH_KEY_UNREGISTERED`, `PEER_ID_INVALID`, `USERNAME_NOT_OCCUPIED`.

---

## 🔧 Raw API Escape Hatch

Any TL function can be invoked directly:

```rust
use layer_tl_types::functions;

// Call any function directly
let state = client.invoke(&functions::updates::GetState {}).await?;

// Call on a specific DC (e.g. for media DCs)
let result = client.invoke_on_dc(&functions::upload::GetFile { ... }, dc_id).await?;
```

---

## 🔌 Shutdown

```rust
// Soft shutdown — signals the background task to stop
let (client, shutdown) = Client::builder()
    .api_id(12345)
    .api_hash("abc123")
    .session("my.session")
    .connect().await?;

// ... use client ...

// Initiate a graceful disconnect
client.disconnect();

// Or hold `shutdown` and drop it when done
drop(shutdown);
```

The `signal_network_restored()` method re-triggers an update catch-up after a network outage:

```rust
client.signal_network_restored();
```

---

## 🔗 Part of the layer stack

```
layer-client        ← you are here
└── layer-mtproto   (session, DH, framing)
    └── layer-tl-types  (generated API types, TL Layer 224)
        └── layer-crypto    (AES-IGE, RSA, SHA, factorize)
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
