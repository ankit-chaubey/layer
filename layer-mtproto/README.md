<div align="center">

# 📡 layer-mtproto

**MTProto 2.0 session management, DH key exchange, and message framing for Rust.**

[![Crates.io](https://img.shields.io/crates/v/layer-mtproto?color=fc8d62)](https://crates.io/crates/layer-mtproto)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-2024_edition-f74c00)](https://www.rust-lang.org/)
[![TL Layer](https://img.shields.io/badge/TL%20Layer-224-8b5cf6)](https://core.telegram.org/mtproto)

*A complete, from-scratch implementation of Telegram's MTProto 2.0 session layer.*

</div>

---

## 📦 Installation

```toml
[dependencies]
layer-mtproto = "0.4.4"
layer-tl-types = { version = "0.4.4", features = ["tl-mtproto"] }
```

---

## ✨ What It Does

`layer-mtproto` implements the full MTProto 2.0 session layer — the encrypted tunnel through which all Telegram API calls travel. This is the core protocol machinery that sits between your application code and the TCP socket.

It handles:
- 🤝 **3-step DH key exchange** — deriving a shared auth key from scratch
- 🔐 **Encrypted sessions** — packing/unpacking MTProto 2.0 messages with AES-IGE
- 📦 **Message framing** — salt, session_id, message_id, sequence numbers
- 🗜️ **Containers & compression** — `msg_container`, gzip-packed responses
- 🧂 **Salt management** — server salt tracking and auto-correction
- 🔄 **Session state** — time offset, sequence number, salt rotation

---

## 🏗️ Architecture

```
Application (layer-client)
       │
       ▼
  EncryptedSession         ← encrypt/decrypt, pack/unpack
       │
       ▼
  Authentication           ← 3-step DH handshake (step1 → step2 → step3 → finish)
       │
       ▼
  layer-crypto             ← AES-IGE, RSA, SHA, Diffie-Hellman
       │
       ▼
  TCP Socket
```

---

## 📚 Core Types

### `EncryptedSession`

Manages the live MTProto session after a key has been established.

```rust
use layer_mtproto::EncryptedSession;

// Create from a completed DH handshake
let session = EncryptedSession::new(auth_key, first_salt, time_offset);

// Pack a RemoteCall into encrypted wire bytes
let wire_bytes = session.pack(&my_request);

// Or pack any Serializable (bypasses RemoteCall bound)
let wire_bytes = session.pack_serializable(&my_request);

// Unpack an encrypted response
let msg = session.unpack(&mut raw_bytes)?;
println!("body: {:?}", msg.body);
```

### `authentication` — DH Key Exchange

The full 3-step DH handshake as specified by MTProto:

```rust
use layer_mtproto::authentication as auth;

// Step 1 — send req_pq_multi
let (req1, state1) = auth::step1()?;
// ... send req1, receive res_pq ...

// Step 2 — send req_DH_params
let (req2, state2) = auth::step2(state1, res_pq)?;
// ... send req2, receive server_DH_params ...

// Step 3 — send set_client_DH_params
let (req3, state3) = auth::step3(state2, dh_params)?;
// ... send req3, receive dh_answer ...

// Finish — extract auth key
let done = auth::finish(state3, dh_answer)?;

// done.auth_key       → [u8; 256]
// done.first_salt     → i64
// done.time_offset    → i32
```

### `Message`

A decoded MTProto message from the server.

```rust
pub struct Message {
    pub salt:    i64,
    pub body:    Vec<u8>,   // raw TL bytes of the inner object
}
```

### `Session`

Plain (unencrypted) session for sending the initial DH handshake messages.

```rust
let mut plain = Session::new();
let framed = plain.pack(&my_plaintext_request).to_plaintext_bytes();
```

---

## 🔍 What's Inside

### Encryption
- AES-IGE encryption/decryption using `layer-crypto`
- `msg_key` derivation from auth key and plaintext body
- Server-side key derivation reversal for decryption

### Framing
Every outgoing MTProto message includes:
```
server_salt     (8 bytes) — current server salt
session_id      (8 bytes) — random, stable per session
message_id      (8 bytes) — time-based, monotonically increasing
seq_no          (4 bytes) — content-related counter
message_length  (4 bytes) — payload length
payload         (N bytes) — serialized TL object
```
Plus 32 bytes of AES-IGE overhead.

### Containers & Compression
- `msg_container` (multiple messages in one frame) supported for both packing and unpacking
- `gzip_packed` outgoing compression for large requests
- `gzip_packed` response decompression

### Salt & Time Management
- `bad_server_salt` — session automatically records the corrected salt
- `future_salts` prefetch — salts are requested in advance and rotated before expiry
- `time_offset` correction — clock skew applied to all outgoing `msg_id` values

### Error Recovery
- `bad_msg_notification` handling — messages are auto-resent with corrected framing
- `seq_no` correction for error codes 32 and 33
- `msg_resend_req` fulfilment from a sent-body cache

### Acknowledgements
- Outgoing `MsgsAck` handling — received content-related messages are acknowledged on a timer
- `pending_ack` system tracks which message IDs still need flushing

---

## 🔗 Part of the layer stack

```
layer-client
└── layer-mtproto     ← you are here
    ├── layer-tl-types
    └── layer-crypto
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
