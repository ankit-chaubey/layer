<div align="center">

# 🤝 layer-client

**High-level async Telegram client for Rust.**

[![Crates.io](https://img.shields.io/crates/v/layer-client?color=fc8d62)](https://crates.io/crates/layer-client)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-2024_edition-f74c00)](https://www.rust-lang.org/)
[![TL Layer](https://img.shields.io/badge/TL%20Layer-222-8b5cf6)](https://core.telegram.org/schema)

*The friendly face of the layer stack — connect, authenticate, send messages, and stream updates with a clean async API.*

</div>

---

## 📦 Installation

```toml
[dependencies]
layer-client = "0.1.2"
tokio = { version = "1", features = ["full"] }
```

---

## ✨ What It Does

`layer-client` wraps the raw MTProto machinery into a clean, ergonomic async API. You don't need to know anything about TL schemas, DH handshakes, or message framing — just connect and go.

- 🔐 **User auth** — phone code + optional 2FA (SRP)
- 🤖 **Bot auth** — bot token login
- 💬 **Messaging** — send, delete, fetch message history
- 📡 **Update stream** — typed async events (NewMessage, CallbackQuery, etc.)
- 🔁 **FLOOD_WAIT retries** — automatic with configurable policy
- 🌐 **DC migration** — handled transparently
- 💾 **Session persistence** — survive restarts without re-login
- 🔧 **Raw API access** — `client.invoke(req)` for any TL function

---

## 🚀 Quick Start — User Account

```rust
use layer_client::{Client, Config, SignInError};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Client::connect(Config {
        session_path: "my.session".into(),
        api_id:       12345,          // https://my.telegram.org
        api_hash:     "abc123".into(),
        ..Default::default()
    }).await?;

    if !client.is_authorized().await? {
        let token = client.request_login_code("+1234567890").await?;

        // read code from user input
        let code = "12345";

        match client.sign_in(&token, code).await {
            Ok(name) => println!("✅ Signed in as {name}"),
            Err(SignInError::PasswordRequired(t)) => {
                let hint = t.hint().unwrap_or("no hint");
                println!("2FA required — hint: {hint}");
                client.check_password(t, "my_password").await?;
            }
            Err(e) => return Err(e.into()),
        }
        client.save_session().await?;
    }

    client.send_message("me", "Hello from layer! 👋").await?;
    Ok(())
}
```

---

## 🤖 Quick Start — Bot

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
                println!("📨 {}", msg.text().unwrap_or("(no text)"));

                if let Some(peer) = msg.peer_id() {
                    client.send_message_to_peer(
                        peer.clone(),
                        &format!("Echo: {}", msg.text().unwrap_or("")),
                    ).await?;
                }
            }
            Update::CallbackQuery(cb) => {
                client.answer_callback_query(cb.query_id, Some("Got it!"), false).await?;
            }
            Update::InlineQuery(iq) => {
                println!("🔍 Inline: {}", iq.query());
            }
            _ => {}
        }
    }

    Ok(())
}
```

---

## 📚 API Reference

### Connection

```rust
// Connect and load/create session
let client = Client::connect(config).await?;

// Check if already logged in
let authed = client.is_authorized().await?;

// Save session to disk
client.save_session().await?;

// Sign out
client.sign_out().await?;
```

### Authentication

```rust
// Phone login
let token = client.request_login_code("+1234567890").await?;
client.sign_in(&token, "12345").await?;

// 2FA
client.check_password(password_token, "my_password").await?;

// Bot login
client.bot_sign_in("token:here").await?;
```

### Messaging

```rust
// By username, "me", or numeric ID
client.send_message("@username", "Hello!").await?;
client.send_message("me", "Saved!").await?;
client.send_to_self("Quick note").await?;

// By resolved peer
client.send_message_to_peer(peer, "text").await?;

// Delete messages
client.delete_messages(vec![123, 456], true).await?;

// Fetch history
let msgs = client.get_messages(peer, 50, 0).await?;
```

### Updates

```rust
let mut stream = client.stream_updates();

while let Some(update) = stream.next().await {
    match update {
        Update::NewMessage(msg)      => { /* new message */ }
        Update::MessageEdited(msg)   => { /* edit */       }
        Update::MessageDeleted(del)  => { /* deletion */   }
        Update::CallbackQuery(cb)    => { /* button press */}
        Update::InlineQuery(iq)      => { /* @bot query */ }
        Update::Raw(raw)             => { /* anything else */}
        _ => {}
    }
}
```

### IncomingMessage

```rust
msg.id()          // → i32
msg.text()        // → Option<&str>
msg.peer_id()     // → Option<&Peer>
msg.sender_id()   // → Option<&Peer>
msg.outgoing()    // → bool
msg.reply(&mut client, "text").await?;
```

### Bots

```rust
// Answer callback query
client.answer_callback_query(cb.query_id, Some("Done!"), false).await?;

// Answer with alert popup
client.answer_callback_query(cb.query_id, Some("Alert!"), true).await?;

// Answer inline query
client.answer_inline_query(iq.query_id, results, 300, false, None).await?;
```

### Peer Resolution

```rust
// Supported formats
client.resolve_peer("me").await?;
client.resolve_peer("@username").await?;
client.resolve_peer("123456789").await?;
```

### Raw API

```rust
// Invoke any TL function directly
let result = client.invoke(&layer_tl_types::functions::updates::GetState {}).await?;
```

---

## ⚙️ Configuration

```rust
Config {
    session_path: "my.session".into(),    // where to store auth key
    api_id:       12345,                   // from https://my.telegram.org
    api_hash:     "abc123".into(),         // from https://my.telegram.org
    dc_addr:      None,                    // override initial DC (default: DC2)
    retry_policy: Arc::new(AutoSleep::default()),  // flood-wait policy
}
```

### Retry Policies

```rust
// Auto-sleep on FLOOD_WAIT (default)
retry_policy: Arc::new(AutoSleep::default())

// Never retry — propagate all errors immediately
retry_policy: Arc::new(NoRetries)

// Custom policy
struct MyPolicy;
impl RetryPolicy for MyPolicy {
    fn should_retry(&self, ctx: &RetryContext) -> ControlFlow<(), Duration> {
        // your logic
    }
}
```

---

## 🔗 Part of the layer stack

```
layer-client        ← you are here
└── layer-mtproto   (session, DH, framing)
    └── layer-tl-types  (generated API types)
        └── layer-crypto    (AES, RSA, SHA)
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
