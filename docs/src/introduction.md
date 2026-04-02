# ⚡ layer

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="../images/layer-banner-dark.png">
  <source media="(prefers-color-scheme: light)" srcset="../images/layer-banner-light.png">
  <img src="../images/layer-banner.png" alt="layer — Async Rust MTProto" width="100%" style="border-radius:8px; margin-bottom:1.5rem;" />
</picture>

<div class="hero-banner">
<h2>A modular, production-grade async Rust implementation of the Telegram MTProto protocol</h2>
<div class="hero-badges">

[![Crates.io](https://img.shields.io/crates/v/layer-client?color=7c6af7&label=layer-client&style=flat-square)](https://crates.io/crates/layer-client)
[![docs.rs](https://img.shields.io/docsrs/layer-client?style=flat-square&color=22c55e)](https://docs.rs/layer-client)
[![TL Layer](https://img.shields.io/badge/TL%20Layer-224-8b5cf6?style=flat-square)](https://core.telegram.org/schema)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue?style=flat-square)](#license)
[![Rust](https://img.shields.io/badge/rust-2024_edition-f74c00?style=flat-square)](https://www.rust-lang.org/)

</div>
</div>

`layer` is a hand-written, bottom-up implementation of [Telegram MTProto](https://core.telegram.org/mtproto) in pure Rust. Every component — from the `.tl` schema parser, to AES-IGE encryption, to the Diffie-Hellman key exchange, to the typed async update stream — is owned and understood by this project.

**No black boxes. No magic. Just Rust, all the way down.**

---

## Why layer?

Most Telegram libraries are thin wrappers around generated code or ports from Python/JavaScript. `layer` is different — it was built from scratch to understand MTProto at the lowest level, then wrapped in an ergonomic high-level API.

<div class="feature-grid">
<div class="feature-card">
<div class="fc-icon">🦀</div>
<div class="fc-title">Pure Rust</div>
<div class="fc-desc">No FFI, no unsafe blocks. Fully async with Tokio. Works on Android (Termux), Linux, macOS, Windows.</div>
</div>
<div class="feature-card">
<div class="fc-icon">⚡</div>
<div class="fc-title">Full MTProto 2.0</div>
<div class="fc-desc">Complete DH handshake, AES-IGE encryption, salt tracking, DC migration — all handled automatically.</div>
</div>
<div class="feature-card">
<div class="fc-icon">🔐</div>
<div class="fc-title">User + Bot Auth</div>
<div class="fc-desc">Phone login with 2FA SRP, bot token login, session persistence across restarts.</div>
</div>
<div class="feature-card">
<div class="fc-icon">📡</div>
<div class="fc-title">Typed Update Stream</div>
<div class="fc-desc">NewMessage, MessageEdited, CallbackQuery, InlineQuery, ChatAction, UserStatus — all strongly typed.</div>
</div>
<div class="feature-card">
<div class="fc-icon">🔧</div>
<div class="fc-title">Raw API Escape Hatch</div>
<div class="fc-desc">Call any of 500+ Telegram API methods directly via <code>client.invoke()</code> with full type safety.</div>
</div>
<div class="feature-card">
<div class="fc-icon">🏗️</div>
<div class="fc-title">Auto-Generated Types</div>
<div class="fc-desc">All 2,329 Layer 224 constructors generated at build time from the official TL schema.</div>
</div>
</div>

---

## Crate overview

<img src="../images/arch-stack.svg" alt="layer crate architecture stack" width="100%" style="margin: 1rem 0 1.5rem 0;" />

| Crate | Description | Typical user |
|---|---|---|
| **`layer-client`** | High-level async client — auth, send, receive, bots | ✅ You |
| `layer-tl-types` | All Layer 224 constructors, functions, enums | Raw API calls |
| `layer-mtproto` | MTProto session, DH, framing, transport | Library authors |
| `layer-crypto` | AES-IGE, RSA, SHA, auth key derivation | Internal |
| `layer-tl-gen` | Build-time Rust code generator | Build tool |
| `layer-tl-parser` | `.tl` schema → AST parser | Build tool |

> **TIP:** Most users only ever import `layer-client`. The other crates are either used internally or for advanced raw API calls.

---

## Quick install

```toml
[dependencies]
layer-client = "0.4.4"
tokio        = { version = "1", features = ["full"] }
```

Then head to [Installation](./installation.md) for credentials setup, or jump straight to:

- [Quick Start — User Account](./quickstart-user.md) — login, send a message, receive updates
- [Quick Start — Bot](./quickstart-bot.md) — bot token login, commands, callbacks

---

## What's new in v0.4.4

- **`StringSessionBackend`** — portable base64 sessions, no file required
- **`LibSqlBackend`** — libsql/Turso remote database sessions
- **`Update::ChatAction`** and **`Update::UserStatus`** — new typed update variants
- **`sync_update_state()`** — force immediate pts/seq reconciliation
- 7 bug fixes (pagination, memory leaks, chunk alignment, and more)

See the full [CHANGELOG](https://github.com/ankit-chaubey/layer/blob/main/CHANGELOG.md).

---

## Acknowledgements

- [**Lonami**](https://codeberg.org/Lonami) for [**grammers**](https://codeberg.org/Lonami/grammers) — the architecture, SRP math, and session design are deeply inspired by this fantastic library.
- [**Telegram**](https://core.telegram.org/mtproto) for the detailed MTProto specification.
- The Rust async ecosystem: `tokio`, `flate2`, `getrandom`, `sha2`, and friends.
