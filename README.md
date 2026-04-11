<div align="center">

# layer

Async Rust library for the Telegram MTProto protocol.
Developed by **[Ankit Chaubey](https://github.com/ankit-chaubey)**.

[![Crates.io](https://img.shields.io/crates/v/layer-client?style=flat-square\&color=fc8d62\&label=layer-client)](https://crates.io/crates/layer-client)
[![docs.rs](https://img.shields.io/badge/docs.rs-layer--client-5865F2?style=flat-square)](https://docs.rs/layer-client)
[![License](https://img.shields.io/badge/license-MIT%20%7C%20Apache--2.0-blue?style=flat-square)](LICENSE-MIT)
[![TL Layer](https://img.shields.io/badge/TL%20Layer-224-8b5cf6?style=flat-square)](https://core.telegram.org/schema)
[![Telegram](https://img.shields.io/badge/chat-%40layer__chat-2CA5E0?style=flat-square\&logo=telegram)](https://t.me/layer_chat)

</div>

> **Pre-production (`0.x.x`)**: APIs may change between minor versions. See [CHANGELOG](CHANGELOG.md) before upgrading.


---

## Crates

Most users only need `layer-client`.

| Crate | Description |
|---|---|
| [`layer-client`](./layer-client) | High-level async client: auth, messaging, media, bots |
| [`layer-tl-types`](./layer-tl-types) | Layer 224 types, functions, enums (2,329 definitions) |
| [`layer-mtproto`](./layer-mtproto) | MTProto session, DH exchange, framing, transports |
| [`layer-crypto`](./layer-crypto) | AES-IGE, RSA, SHA, Diffie-Hellman, auth key derivation |
| [`layer-tl-gen`](./layer-tl-gen) | Build-time code generator from the TL AST |
| [`layer-tl-parser`](./layer-tl-parser) | Parses `.tl` schema into an AST |

---

## Installation

```toml
[dependencies]
layer-client = "0.4.9"
tokio        = { version = "1", features = ["full"] }
```

Get your `api_id` and `api_hash` from [my.telegram.org](https://my.telegram.org).

Optional features:

```toml
layer-client = { version = "0.4.9", features = ["sqlite-session"] }  # SQLite session
layer-client = { version = "0.4.9", features = ["libsql-session"] }  # libsql / Turso
layer-client = { version = "0.4.9", features = ["html"] }            # HTML parser
layer-client = { version = "0.4.9", features = ["html5ever"] }       # html5ever parser
```

`layer-client` re-exports `layer_tl_types` as `layer_client::tl`.

---

## Quick Start - Bot

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
                client.send_message_to_peer(peer.clone(), msg.text().unwrap_or("")).await?;
            }
        }
    }
    Ok(())
}
```

## Quick Start - User Account

```rust
use layer_client::{Client, SignInError};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (client, _shutdown) = Client::builder()
        .api_id(12345)
        .api_hash("your_api_hash")
        .session("my.session")
        .connect()
        .await?;

    if !client.is_authorized().await? {
        let token = client.request_login_code("+1234567890").await?;
        let code  = read_line();

        match client.sign_in(&token, &code).await {
            Ok(name) => println!("Welcome, {name}!"),
            Err(SignInError::PasswordRequired(t)) => {
                client.check_password(*t, "my_2fa_password").await?;
            }
            Err(e) => return Err(e.into()),
        }
        client.save_session().await?;
    }

    client.send_message("me", "Hello from layer!").await?;
    Ok(())
}
```

---

## Session Backends

| Backend | Flag | Notes |
|---|---|---|
| `BinaryFileBackend` | default | Single-process bots, scripts |
| `InMemoryBackend` | default | Tests, ephemeral tasks |
| `StringSessionBackend` | default | Serverless, env-var storage |
| `SqliteBackend` | `sqlite-session` | Multi-session local apps |
| `LibSqlBackend` | `libsql-session` | Turso / distributed storage |
| Custom | - | Implement `SessionBackend` |

```rust
let s = client.export_session_string().await?;
let (client, _) = Client::with_string_session(&s).await?;
```

---

## Raw API

Every Layer method is accessible via `client.invoke()`:

```rust
use layer_client::tl;

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

// Target a specific DC
client.invoke_on_dc(&req, 2).await?;
```

---

## Tests

```bash
cargo test --workspace
cargo test --workspace --all-features
```

Integration tests in `layer-client/tests/integration.rs` use `InMemoryBackend` and don't need real credentials.

---

## Community

- Channel: [t.me/layer_rs](https://t.me/layer_rs)
- Chat: [t.me/layer_chat](https://t.me/layer_chat)
- Guide: [layer.ankitchaubey.in](https://layer.ankitchaubey.in/)
- API docs: [docs.rs/layer-client](https://docs.rs/layer-client)

---

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a PR. Run `cargo test --workspace` and `cargo clippy --workspace` locally. Security issues: see [SECURITY.md](SECURITY.md).

---

## Author

Developed by [**Ankit Chaubey**](https://github.com/ankit-chaubey) out of curiosity to explore.

Layer is developed as part of exploration, learning, and experimentation with the Telegram MTProto protocol.
Use it at your own risk. Its future and stability are not yet guaranteed.

---

## Acknowledgements

Parts of this library are derived from [grammers](https://codeberg.org/Lonami/grammers) by [Lonami](https://codeberg.org/Lonami), licensed under MIT or Apache-2.0.

AI was used to reduce manual effort on docs, boilerplate, and large rewrites where it made sense. Core library code is written by the author.

---

## License

Licensed under either of, at your option:

- MIT License: see [LICENSE-MIT](LICENSE-MIT)
- Apache License, Version 2.0: see [LICENSE-APACHE](LICENSE-APACHE)

Unless you explicitly state otherwise, any contribution submitted for inclusion shall be dual-licensed as above, without any additional terms or conditions.

---

## Telegram Terms of Service

Ensure your usage complies with [Telegram's Terms of Service](https://core.telegram.org/api/terms) and [API Terms of Service](https://core.telegram.org/api/terms). Misuse of the Telegram API, including spam, mass scraping, or automation of normal user accounts, may result in account limitations or permanent bans.
