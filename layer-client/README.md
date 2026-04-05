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
- [Feature Flags](#-feature-flags)
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
- [Peer Types](#-peer-types)
- [Peer Resolution & PeerRef](#-peer-resolution--peerref)
- [Session Backends](#-session-backends)
- [Transport & Networking](#-transport--networking)
- [Feature Flags](#-feature-flags)
- [Configuration Reference](#-configuration-reference)
- [Error Handling](#-error-handling)
- [Raw API Escape Hatch](#-raw-api-escape-hatch)
- [Shutdown](#-shutdown)
- [Client Methods — Full Reference](#client-methods--full-reference)

---

## 📦 Installation

```toml
[dependencies]
layer-client = "0.4.6"
tokio        = { version = "1", features = ["full"] }
```

Get your `api_id` and `api_hash` from **[my.telegram.org](https://my.telegram.org)**.

---

## 🚩 Feature Flags

```toml
# SQLite session persistence
layer-client = { version = "0.4.6", features = ["sqlite-session"] }

# libsql / Turso session (local or remote)
layer-client = { version = "0.4.6", features = ["libsql-session"] }

# Hand-rolled HTML parser
layer-client = { version = "0.4.6", features = ["html"] }

# Spec-compliant html5ever parser (replaces built-in)
layer-client = { version = "0.4.6", features = ["html5ever"] }
```

`StringSessionBackend`, `InMemoryBackend`, and `BinaryFileBackend` are always available — no flag needed.

---

## ⚡ Minimal Bot — 15 Lines

```rust
use layer_client::{Client, update::Update};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (client, _shutdown) = Client::builder()
        .api_id(std::env::var("API_ID")?.parse()?)
        .api_hash(std::env::var("API_HASH")?)
        .session("bot.session")
        .connect()
        .await?;

    client.bot_sign_in(&std::env::var("BOT_TOKEN")?).await?;
    client.save_session().await?;

    let mut stream = client.stream_updates();
    while let Some(Update::NewMessage(msg)) = stream.next().await {
        if !msg.outgoing() {
            if let Some(peer) = msg.peer_id() {
                client.send_message_to_peer(peer.clone(), "Echo!").await?;
            }
        }
    }
    Ok(())
}
```

---

## 🔨 Connecting — ClientBuilder

```rust
use layer_client::Client;

let (client, _shutdown) = Client::builder()
    .api_id(12345)
    .api_hash("your_api_hash")
    .session("my.session")     // BinaryFileBackend
    .catch_up(true)            // replay missed updates on reconnect
    .connect()
    .await?;
```

### `ClientBuilder` methods

| Method | Description |
|---|---|
| `.api_id(i32)` | Telegram API ID (required) |
| `.api_hash(str)` | Telegram API hash (required) |
| `.session(path)` | Binary file session at `path` |
| `.session_string(s)` | Portable base64 string session |
| `.in_memory()` | Non-persistent in-memory session (tests) |
| `.session_backend(Arc<dyn SessionBackend>)` | Inject a custom backend |
| `.catch_up(bool)` | Replay missed updates on connect (default: false) |
| `.dc_addr(str)` | Override first DC address |
| `.socks5(Socks5Config)` | Route all connections through a SOCKS5 proxy |
| `.allow_ipv6(bool)` | Allow IPv6 DC addresses (default: false) |
| `.transport(TransportKind)` | MTProto transport (default: Abridged) |
| `.retry_policy(Arc<dyn RetryPolicy>)` | Override flood-wait retry policy |
| `.build()` | Build `Config` without connecting |
| `.connect()` | Build and connect — returns `(Client, ShutdownToken)` |

---

## 🔑 Authentication

### Bot login

```rust
if !client.is_authorized().await? {
    client.bot_sign_in("1234567890:ABCdef...").await?;
    client.save_session().await?;
}
```

### User login (phone + code + optional 2FA)

```rust
use layer_client::SignInError;

if !client.is_authorized().await? {
    let phone = "+1234567890";
    let token = client.request_login_code(phone).await?;

    print!("Enter code: ");
    let code = read_line();

    match client.sign_in(&token, &code).await {
        Ok(name) => println!("Welcome, {name}!"),
        Err(SignInError::PasswordRequired(t)) => {
            client.check_password(*t, "my_2fa_password").await?;
        }
        Err(e) => return Err(e.into()),
    }
    client.save_session().await?;
}
```

### Sign out

```rust
client.sign_out().await?;
```

---

## 🔑 String Sessions — Portable Auth

```rust
// Export — works from any running client
let session_string = client.export_session_string().await?;
std::env::set_var("TG_SESSION", &session_string);

// Restore — no phone/code flow needed
let session = std::env::var("TG_SESSION").unwrap_or_default();
let (client, _shutdown) = Client::builder()
    .api_id(12345)
    .api_hash("your_api_hash")
    .session_string(session)
    .connect()
    .await?;
```

---

## 📡 Update Stream

```rust
let mut stream = client.stream_updates();
while let Some(update) = stream.next().await {
    match update {
        Update::NewMessage(msg)    => { /* new message */ }
        Update::MessageEdited(msg) => { /* edited message */ }
        Update::MessageDeleted(d)  => { /* deleted messages */ }
        Update::CallbackQuery(cb)  => { /* inline button pressed */ }
        Update::InlineQuery(iq)    => { /* @bot inline mode */ }
        Update::InlineSend(is)     => { /* user selected inline result */ }
        Update::UserTyping(a)      => { /* typing / uploading */ }
        Update::UserStatus(s)      => { /* online/offline status */ }
        Update::Raw(raw)           => { /* unmapped TL update */ }
        _ => {}  // always add a fallback — Update is #[non_exhaustive]
    }
}
```

### `IncomingMessage` accessors

```rust
Update::NewMessage(msg) => {
    msg.id()        // i32
    msg.text()      // Option<&str>
    msg.peer_id()   // Option<&tl::enums::Peer>
    msg.sender_id() // Option<&tl::enums::Peer>
    msg.outgoing()  // bool
    msg.date()      // i32 — Unix timestamp
    msg.edit_date() // Option<i32>
    msg.mentioned() // bool
    msg.silent()    // bool
    msg.pinned()    // bool
    msg.post()      // bool — channel post
    msg.raw         // tl::enums::Message — full TL object
}
```

---

## 💬 Messaging

```rust
// Any peer format works — PeerRef accepts &str, i64, tl::enums::Peer
client.send_message("@username", "Hello!").await?;
client.send_message("me", "Note to self").await?;
client.send_to_self("Reminder 📝").await?;

// From an incoming message
if let Some(peer) = msg.peer_id() {
    client.send_message_to_peer(peer.clone(), "Reply!").await?;
}

// Edit
client.edit_message(peer.clone(), msg_id, "Updated text").await?;

// Forward
client.forward_messages(from_peer.clone(), to_peer.clone(), &[id1, id2]).await?;

// Delete
client.delete_messages(peer.clone(), &[id1, id2]).await?;

// Pin / unpin
client.pin_message(peer.clone(), msg_id, true).await?;   // notify = true
client.unpin_message(peer.clone(), msg_id).await?;
client.unpin_all_messages(peer.clone()).await?;

// Get pinned message
let pinned = client.get_pinned_message(peer.clone()).await?;

// Get reply-to message
let replied_to = client.get_reply_to_message(peer.clone(), msg_id).await?;

// Scheduled messages
let scheduled = client.get_scheduled_messages(peer.clone()).await?;
client.delete_scheduled_messages(peer.clone(), &[sched_id]).await?;
```

---

## 📝 InputMessage Builder

```rust
use layer_client::{InputMessage, parsers::parse_markdown};
use layer_client::keyboard::{Button, InlineKeyboard};

let (text, entities) = parse_markdown("**Bold** and `code`");

let kb = InlineKeyboard::new()
    .row([
        Button::callback("✅ Yes", b"confirm:yes"),
        Button::callback("❌ No",  b"confirm:no"),
    ])
    .row([Button::url("📖 Docs", "https://docs.rs/layer-client")]);

client
    .send_message_to_peer_ex(
        peer.clone(),
        &InputMessage::text(text)
            .entities(entities)
            .reply_to(Some(msg_id))
            .silent(true)
            .no_webpage(true)
            .keyboard(kb),
    )
    .await?;
```

### `InputMessage` builder methods

| Method | Description |
|---|---|
| `InputMessage::text(str)` | Create with text |
| `.set_text(str)` | Change the text |
| `.reply_to(Option<i32>)` | Reply to message ID |
| `.silent(bool)` | No notification sound |
| `.background(bool)` | Send in background |
| `.clear_draft(bool)` | Clear the draft |
| `.no_webpage(bool)` | Suppress link preview |
| `.invert_media(bool)` | Show media above caption (Telegram ≥ 10.3) |
| `.schedule_once_online()` | Send when recipient goes online |
| `.schedule_date(Option<i32>)` | Schedule for Unix timestamp |
| `.entities(Vec<MessageEntity>)` | Formatted text entities |
| `.reply_markup(ReplyMarkup)` | Raw TL reply markup |
| `.keyboard(impl Into<ReplyMarkup>)` | `InlineKeyboard` or `ReplyKeyboard` |
| `.copy_media(InputMedia)` | Attach media from an existing message |
| `.clear_media()` | Remove attached media |

---

## ⌨️ Keyboards

### Inline keyboard (triggers `CallbackQuery`)

```rust
use layer_client::keyboard::{Button, InlineKeyboard};

let kb = InlineKeyboard::new()
    .row([
        Button::callback("👍 Like",  b"vote:like"),
        Button::callback("👎 Dislike", b"vote:dislike"),
    ])
    .row([
        Button::url("🔗 Docs", "https://docs.rs/layer-client"),
        Button::copy_text("📋 Copy token", "abc123"),
    ]);

client
    .send_message_to_peer_ex(peer.clone(), &InputMessage::text("Vote!").keyboard(kb))
    .await?;
```

**All button types:** `callback`, `url`, `url_auth`, `switch_inline`, `switch_elsewhere`,
`webview`, `simple_webview`, `request_phone`, `request_geo`, `request_poll`, `request_quiz`,
`game`, `buy`, `copy_text`, `text` (reply keyboards only).

### Reply keyboard

```rust
use layer_client::keyboard::{Button, ReplyKeyboard};

let kb = ReplyKeyboard::new()
    .row([Button::text("📸 Photo"), Button::text("📄 Document")])
    .row([Button::text("❌ Cancel")])
    .resize()
    .single_use();

client
    .send_message_to_peer_ex(peer.clone(), &InputMessage::text("Choose:").keyboard(kb))
    .await?;
```

### Answer callback queries

```rust
Update::CallbackQuery(cb) => {
    client.answer_callback_query(cb.query_id, Some("✅ Done!"), false).await?;
    // Pass alert: true for a popup alert
}
```

---

## 📎 Media Upload & Download

```rust
// Upload from bytes
let uploaded = client.upload_file("photo.jpg", &bytes).await?;

// Upload — parallel chunks (faster for large files)
let uploaded = client.upload_file_concurrent("video.mp4", &bytes).await?;

// Upload from async reader
use tokio::fs::File;
let f = File::open("document.pdf").await?;
let uploaded = client.upload_stream("document.pdf", f).await?;

// Send file (false = as document, true = as photo/media)
client.send_file(peer.clone(), uploaded, false).await?;

// Send album
client.send_album(peer.clone(), vec![uploaded_a, uploaded_b]).await?;

// Download to bytes
let bytes = client.download_media(&msg_media).await?;

// Download to file
client.download_media_to_file(&msg_media, "output.jpg").await?;

// Use the Downloadable trait (Photo, Document, Sticker)
use layer_client::media::{Photo, Downloadable};
let photo = Photo::from_media(&msg.raw)?;
let bytes = client.download(&photo).await?;
```

---

## 🖊️ Text Formatting

```rust
use layer_client::parsers::{parse_markdown, generate_markdown};

let (text, entities) = parse_markdown(
    "**Bold**, `code`, _italic_, [link](https://example.com)"
);
client
    .send_message_to_peer_ex(peer.clone(), &InputMessage::text(text).entities(entities))
    .await?;

let md = generate_markdown(&plain_text, &entities);
```

With the `html` feature:

```rust
use layer_client::parsers::{parse_html, generate_html};

let (text, entities) = parse_html("<b>Bold</b> and <code>mono</code>");
let html_str = generate_html(&plain_text, &entities);
```

---

## 💥 Reactions

```rust
use layer_client::reactions::InputReactions;

// Simple emoji (&str converts automatically)
client.send_reaction(peer.clone(), msg_id, "👍").await?;

// Custom (premium) emoji
client.send_reaction(peer.clone(), msg_id, InputReactions::custom_emoji(doc_id)).await?;

// Big animated reaction
client.send_reaction(peer.clone(), msg_id, InputReactions::emoticon("🔥").big()).await?;

// Remove all reactions
client.send_reaction(peer.clone(), msg_id, InputReactions::remove()).await?;
```

---

## ⌛ Typing Guard — RAII

`TypingGuard` keeps a typing/uploading indicator alive and cancels it on drop:

```rust
// Convenience methods on Client
let _typing = client.typing(peer.clone()).await?;
let _typing = client.uploading_document(peer.clone()).await?;
let _typing = client.recording_video(peer.clone()).await?;
let _typing = client.typing_in_topic(peer.clone(), topic_id).await?;

// Any SendMessageAction via TypingGuard::start
use layer_client::TypingGuard;
let _guard = TypingGuard::start(
    client, peer.clone(),
    tl::enums::SendMessageAction::SendMessageRecordAudioAction,
).await?;

// Forum topic with custom repeat delay
let _guard = TypingGuard::start_ex(
    client, peer, action, Some(topic_id), Duration::from_secs(4)
).await?;

// Cancel immediately without waiting for drop
guard.cancel();
```

---

## 👥 Participants & Chat Management

```rust
use layer_client::participants::{AdminRightsBuilder, BannedRightsBuilder};

// Fetch members
let members = client.get_participants(peer.clone(), 100).await?;

// Lazy iterator
let mut iter = client.iter_participants(peer.clone());
while let Some(p) = iter.next(&client).await? {
    println!("{}", p.user.first_name.as_deref().unwrap_or("?"));
}

// Kick (basic group)
client.kick_participant(peer.clone(), user_id).await?;

// Ban with granular rights
client
    .ban_participant(peer.clone(), user_id,
        BannedRightsBuilder::new().send_media(true).send_stickers(true))
    .await?;

// Permanent full ban
client
    .ban_participant(peer.clone(), user_id, BannedRightsBuilder::full_ban())
    .await?;

// Promote admin
client
    .promote_participant(peer.clone(), user_id,
        AdminRightsBuilder::new()
            .delete_messages(true)
            .ban_users(true)
            .rank("Moderator"))
    .await?;

// Get permissions
let perms = client.get_permissions(peer.clone(), user_id).await?;

// Profile photos
let mut iter = client.iter_profile_photos(user_id);
while let Some(photo) = iter.next(&client).await? {
    let bytes = client.download(&photo).await?;
}
```

---

## 🔍 Search

```rust
use layer_tl_types::enums::MessagesFilter;

// In-chat search
let results = client
    .search(peer.clone(), "hello world")
    .min_date(1_700_000_000)
    .filter(MessagesFilter::InputMessagesFilterPhotos)
    .limit(50)
    .fetch(&client)
    .await?;

// Only messages sent by me
let mine = client.search(peer.clone(), "").sent_by_self().fetch(&client).await?;

// Global search (all chats)
let global = client
    .search_global_builder("rust async")
    .broadcasts_only(true)
    .limit(30)
    .fetch(&client)
    .await?;
```

---

## 📜 Dialogs & Iterators

```rust
// Fetch first N dialogs
let dialogs = client.get_dialogs(50).await?;
for d in &dialogs {
    println!("{} — {} unread", d.title(), d.unread_count());
}

// Lazy dialog iterator
let mut iter = client.iter_dialogs();
while let Some(dialog) = iter.next(&client).await? {
    println!("{}", dialog.title());
}

// Lazy message iterator for a peer
let mut iter = client.iter_messages(peer.clone());
while let Some(msg) = iter.next(&client).await? {
    println!("{}", msg.message);
}

// Fetch by ID
let messages = client.get_messages_by_id(peer.clone(), &[100, 101, 102]).await?;
let latest   = client.get_messages(peer.clone(), 20).await?;

// Read / clear
client.mark_as_read(peer.clone()).await?;
client.clear_mentions(peer.clone()).await?;
client.delete_dialog(peer.clone()).await?;
```

---

## 🧑 Peer Types

High-level wrappers over raw TL types — no constant pattern-matching:

```rust
use layer_client::types::{User, Group, Channel, ChannelKind, Chat};

// User
if let Some(user) = User::from_raw(raw) {
    println!("{}", user.full_name());   // "First Last"
    println!("{:?}", user.username()); // Some("handle")
    println!("bot: {}", user.bot());
    println!("premium: {}", user.premium());
    let peer = user.as_peer();
    let input = user.as_input_peer();
}

// Channel / supergroup
if let Some(ch) = Channel::from_raw(raw) {
    match ch.kind() {
        ChannelKind::Broadcast => println!("Channel"),
        ChannelKind::Megagroup => println!("Supergroup"),
        ChannelKind::Gigagroup => println!("Broadcast group"),
    }
    let input_ch = ch.as_input_channel();
}

// Unified Chat (Group or Channel)
if let Some(chat) = Chat::from_raw(raw) {
    println!("{} — {}", chat.id(), chat.title());
}
```

---

## 🔗 Peer Resolution & PeerRef

Every method that takes a peer accepts `impl Into<PeerRef>`:

```rust
// All of these work anywhere a peer is expected:
client.send_message_to_peer("@username", "hi").await?;
client.send_message_to_peer("me",        "hi").await?;
client.send_message_to_peer(12345678_i64,"hi").await?;
client.send_message_to_peer(-1001234567890_i64, "hi").await?; // Bot-API channel ID
client.send_message_to_peer(tl_peer_value,    "hi").await?;  // already resolved

// Explicit resolution
let peer = client.resolve_peer("@telegram").await?;
let peer = client.resolve_peer("+1234567890").await?;
let peer = client.resolve_username("telegram").await?;
```

---

## 💾 Session Backends

```rust
// Binary file (default)
Client::builder().session("bot.session")

// In-memory (tests)
Client::builder().in_memory()

// String session (serverless, env var)
Client::builder().session_string(std::env::var("TG_SESSION")?)

// SQLite (feature = "sqlite-session")
Client::builder().session_backend(Arc::new(SqliteBackend::new("sessions.db")))

// LibSql / Turso (feature = "libsql-session")
Client::builder().session_backend(Arc::new(LibSqlBackend::remote(url, token)))
```

---

## 🚂 Transport & Networking

```rust
use layer_client::TransportKind;
use layer_client::Socks5Config;

// Transport (default: Abridged)
Client::builder().transport(TransportKind::Intermediate)
Client::builder().transport(TransportKind::Obfuscated) // DPI bypass

// SOCKS5 proxy — no auth
Client::builder().socks5(Socks5Config::new("127.0.0.1:1080"))

// SOCKS5 proxy — with auth
Client::builder().socks5(Socks5Config::with_auth("proxy.host:1080", "user", "pass"))

// Force request to a specific DC
client.invoke_on_dc(&req, 2).await?;

// Signal network restored (skips exponential backoff)
client.signal_network_restored();
```

---

## ⚠️ Error Handling

```rust
use layer_client::{InvocationError, RpcError};

match client.send_message("@peer", "Hello").await {
    Ok(()) => {}
    Err(InvocationError::Rpc(RpcError { code, message, .. })) => {
        eprintln!("Telegram error {code}: {message}");
    }
    Err(InvocationError::Io(e)) => eprintln!("I/O error: {e}"),
    Err(e) => eprintln!("Other: {e}"),
}
```

`FLOOD_WAIT` is handled automatically by the default `AutoSleep` retry policy. To disable:

```rust
use layer_client::retry::NoRetries;
Client::builder().retry_policy(Arc::new(NoRetries))
```

---

## 🔩 Raw API Escape Hatch

Every Layer 224 method (2,329 total) is accessible:

```rust
use layer_client::tl;

let req = tl::functions::bots::SetBotCommands {
    scope:     tl::enums::BotCommandScope::Default(tl::types::BotCommandScopeDefault {}),
    lang_code: "en".into(),
    commands:  vec![
        tl::enums::BotCommand::BotCommand(tl::types::BotCommand {
            command:     "start".into(),
            description: "Start the bot".into(),
        }),
    ],
};
client.invoke(&req).await?;
```

---

## 🛑 Shutdown

```rust
let (client, shutdown) = Client::connect(config).await?;

// Graceful shutdown from any task
shutdown.cancel();

// Immediate disconnect
client.disconnect();
```

`ShutdownToken` is a `CancellationToken` wrapper — clone and pass to multiple tasks.

---

# Client Methods — Full Reference

All `Client` methods. Every `async` method returns `Result<T, InvocationError>` unless noted.

---

## Connection & Session

| Method | Signature | Description |
|---|---|---|
| `Client::builder()` | `→ ClientBuilder` | Fluent builder (recommended) |
| `Client::connect()` | `async (Config) → (Client, ShutdownToken)` | Low-level connect |
| `client.is_authorized()` | `async → bool` | Check if logged in |
| `client.save_session()` | `async → ()` | Persist current session |
| `client.export_session_string()` | `async → String` | Export portable base64 session |
| `client.disconnect()` | `sync` | Immediate disconnect |
| `client.signal_network_restored()` | `sync` | Skip reconnect backoff |
| `client.sync_update_state()` | `async → ()` | Sync update sequence numbers |

---

## Authentication

| Method | Signature | Description |
|---|---|---|
| `client.bot_sign_in(token)` | `async → String` | Bot token login |
| `client.request_login_code(phone)` | `async → LoginToken` | Start user login |
| `client.sign_in(token, code)` | `async → String` | Complete user login |
| `client.check_password(token, pw)` | `async → String` | Submit 2FA password |
| `client.sign_out()` | `async → bool` | Log out |
| `client.get_me()` | `async → tl::types::User` | Get own user info |
| `client.get_users_by_id(ids)` | `async → Vec<Option<User>>` | Fetch users by ID |

---

## Updates

| Method | Signature | Description |
|---|---|---|
| `client.stream_updates()` | `sync → UpdateStream` | Typed async update stream |
| `stream.next()` | `async → Option<Update>` | Next typed update |
| `stream.next_raw()` | `async → Option<RawUpdate>` | Next raw TL update |

---

## Messaging

| Method | Signature | Description |
|---|---|---|
| `client.send_message(peer, text)` | `async → ()` | Send text |
| `client.send_message_to_peer(peer, text)` | `async → ()` | Send text (explicit peer) |
| `client.send_message_to_peer_ex(peer, msg)` | `async → ()` | Send `InputMessage` |
| `client.send_to_self(text)` | `async → ()` | Send to Saved Messages |
| `client.edit_message(peer, id, text)` | `async → ()` | Edit a message |
| `client.edit_inline_message(id, msg)` | `async → ()` | Edit an inline message |
| `client.forward_messages(from, to, ids)` | `async → ()` | Forward messages |
| `client.forward_messages_returning(from, to, ids)` | `async → Vec<Message>` | Forward + return new messages |
| `client.delete_messages(peer, ids)` | `async → ()` | Delete messages |
| `client.get_messages(peer, limit)` | `async → Vec<Message>` | Fetch latest messages |
| `client.get_messages_by_id(peer, ids)` | `async → Vec<Option<Message>>` | Fetch by ID |
| `client.get_pinned_message(peer)` | `async → Option<Message>` | Get pinned message |
| `client.pin_message(peer, id, notify)` | `async → ()` | Pin a message |
| `client.unpin_message(peer, id)` | `async → ()` | Unpin a message |
| `client.unpin_all_messages(peer)` | `async → ()` | Unpin all messages |
| `client.get_reply_to_message(peer, id)` | `async → Option<Message>` | Get message replied to |
| `client.get_scheduled_messages(peer)` | `async → Vec<Message>` | List scheduled messages |
| `client.delete_scheduled_messages(peer, ids)` | `async → ()` | Cancel scheduled messages |

---

## Inline Mode (bots)

| Method | Signature | Description |
|---|---|---|
| `client.answer_inline_query(qid, results, cache, personal, next)` | `async → ()` | Answer inline query |
| `client.answer_callback_query(qid, text, alert)` | `async → ()` | Answer callback query |

---

## Media

| Method | Signature | Description |
|---|---|---|
| `client.upload_file(name, bytes)` | `async → UploadedFile` | Upload (sequential) |
| `client.upload_file_concurrent(name, bytes)` | `async → UploadedFile` | Upload (parallel chunks) |
| `client.upload_stream(name, reader)` | `async → UploadedFile` | Upload from `AsyncRead` |
| `client.send_file(peer, file, as_photo)` | `async → ()` | Send uploaded file |
| `client.send_album(peer, files)` | `async → ()` | Send multiple files as album |
| `client.download_media(loc)` | `async → Vec<u8>` | Download to memory |
| `client.download_media_to_file(loc, path)` | `async → ()` | Stream download to file |
| `client.download_media_concurrent(loc)` | `async → Vec<u8>` | Parallel download |
| `client.download(item)` | `async → Vec<u8>` | Download `Downloadable` (Photo/Doc/Sticker) |
| `client.iter_download(location)` | `sync → DownloadIter` | Lazy chunk iterator |

---

## Chat Actions

| Method | Signature | Description |
|---|---|---|
| `client.send_chat_action(peer, action, topic)` | `async → ()` | One-shot chat action |
| `client.typing(peer)` | `async → TypingGuard` | RAII typing indicator |
| `client.uploading_document(peer)` | `async → TypingGuard` | RAII upload indicator |
| `client.recording_video(peer)` | `async → TypingGuard` | RAII video recording indicator |
| `client.typing_in_topic(peer, topic_id)` | `async → TypingGuard` | RAII typing in forum topic |
| `client.mark_as_read(peer)` | `async → ()` | Mark all messages as read |
| `client.clear_mentions(peer)` | `async → ()` | Clear @mention badges |

---

## Reactions

| Method | Signature | Description |
|---|---|---|
| `client.send_reaction(peer, msg_id, reaction)` | `async → ()` | React / unreact; accepts `&str` or `InputReactions` |

---

## Dialogs & Peers

| Method | Signature | Description |
|---|---|---|
| `client.get_dialogs(limit)` | `async → Vec<Dialog>` | Fetch dialogs |
| `client.iter_dialogs()` | `sync → DialogIter` | Lazy dialog iterator |
| `client.iter_messages(peer)` | `sync → MessageIter` | Lazy message iterator |
| `client.delete_dialog(peer)` | `async → ()` | Leave and delete dialog |
| `client.join_chat(peer)` | `async → ()` | Join public chat |
| `client.accept_invite_link(link)` | `async → ()` | Accept invite link |
| `Client::parse_invite_hash(link)` | `sync → Option<&str>` | Extract hash from link |
| `client.resolve_peer(str)` | `async → Peer` | Resolve username / phone / "me" |
| `client.resolve_username(str)` | `async → Peer` | Resolve bare username |
| `client.resolve_to_input_peer(str)` | `async → InputPeer` | Resolve to `InputPeer` |

---

## Search

| Method | Signature | Description |
|---|---|---|
| `client.search(peer, query)` | `sync → SearchBuilder` | In-chat search builder |
| `client.search_messages(peer, q, limit)` | `async → Vec<IncomingMessage>` | Quick in-chat search |
| `client.search_global_builder(query)` | `sync → GlobalSearchBuilder` | Global search builder |
| `client.search_global(q, limit)` | `async → Vec<IncomingMessage>` | Quick global search |

---

## Participants

| Method | Signature | Description |
|---|---|---|
| `client.get_participants(peer, limit)` | `async → Vec<Participant>` | Fetch members |
| `client.iter_participants(peer)` | `sync → impl Iterator` | Lazy member iterator |
| `client.search_peer(query)` | `async → Vec<Peer>` | Search contacts/dialogs |
| `client.kick_participant(peer, user_id)` | `async → ()` | Kick from basic group |
| `client.ban_participant(peer, user_id, rights)` | `async → ()` | Ban/restrict with `BannedRightsBuilder` |
| `client.promote_participant(peer, user_id, rights)` | `async → ()` | Promote with `AdminRightsBuilder` |
| `client.get_permissions(peer, user_id)` | `async → ParticipantPermissions` | Check user's effective permissions |
| `client.get_profile_photos(user_id, offset, limit)` | `async → Vec<Photo>` | Fetch profile photos |
| `client.iter_profile_photos(user_id)` | `sync → ProfilePhotoIter` | Lazy profile photo iterator |

---

## Raw & Advanced

| Method | Signature | Description |
|---|---|---|
| `client.invoke(req)` | `async → R::Return` | Call any Layer 224 TL method |
| `client.invoke_on_dc(req, dc_id)` | `async → R::Return` | Call on a specific DC |
| `client.cache_users_slice_pub(users)` | `async → ()` | Manually populate peer cache |
| `client.cache_chats_slice_pub(chats)` | `async → ()` | Manually populate peer cache |
| `client.rpc_call_raw_pub(req)` | `async → Vec<u8>` | Raw RPC bytes |
