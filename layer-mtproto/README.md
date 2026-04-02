<div align="center">

<img src="https://raw.githubusercontent.com/ankit-chaubey/layer/main/docs/images/crate-mtproto-banner.svg" alt="layer-mtproto" width="100%" />

# 📡 layer-mtproto

**MTProto 2.0 session management, DH key exchange, and message framing for Rust.**

[![Crates.io](https://img.shields.io/crates/v/layer-mtproto?color=fc8d62)](https://crates.io/crates/layer-mtproto)
[![docs.rs](https://img.shields.io/badge/docs.rs-layer--mtproto-5865F2)](https://docs.rs/layer-mtproto)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-2024_edition-f74c00)](https://www.rust-lang.org/)
[![TL Layer](https://img.shields.io/badge/TL%20Layer-224-8b5cf6)](https://core.telegram.org/mtproto)

*A complete, from-scratch implementation of Telegram's MTProto 2.0 session layer.*

</div>

---

## 📦 Installation

```toml
[dependencies]
layer-mtproto  = "0.4.5"
layer-tl-types = { version = "0.4.5", features = ["tl-mtproto"] }
```

---

## ✨ What It Does

`layer-mtproto` implements the full MTProto 2.0 session layer — the encrypted tunnel through which all Telegram API calls travel. This is the core protocol machinery that sits between your application code and the TCP socket.

It handles:
- 🤝 **3-step DH key exchange** — deriving a shared auth key from scratch
- 🔐 **Encrypted sessions** — packing/unpacking MTProto 2.0 messages with AES-IGE
- 📦 **Message framing** — salt, session_id, message_id, sequence numbers
- 🗜️ **Containers & compression** — `msg_container`, `gzip_packed` responses
- 🧂 **Salt management** — server salt tracking and auto-correction
- 🔄 **Session state** — time offset, sequence number, salt rotation
- 🔁 **Error recovery** — `bad_msg_notification`, `bad_server_salt`, `msg_resend_req`
- ✅ **Acknowledgements** — `MsgsAck` with a pending-ack queue

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
let wire_bytes = session.pack(&my_request)?;

// Pack any Serializable directly (bypasses RemoteCall bound)
let wire_bytes = session.pack_serializable(&raw_obj)?;

// Unpack an encrypted response from the server
let msg = session.unpack(&mut raw_bytes)?;
println!("msg_id={}, body_len={}", msg.msg_id, msg.body.len());
```

---

### `authentication` — 3-Step DH Key Exchange

The full MTProto DH handshake as specified by Telegram:

```rust
use layer_mtproto::authentication as auth;

// Step 1 — req_pq_multi: get the server's PQ
let (req1, state1) = auth::step1()?;
// ... send req1 over the wire, receive res_pq ...

// Step 2 — req_DH_params: send our DH parameters
let (req2, state2) = auth::step2(state1, res_pq)?;
// ... send req2, receive server_DH_params ...

// Step 3 — set_client_DH_params: send our client DH
let (req3, state3) = auth::step3(state2, dh_params)?;
// ... send req3, receive dh_answer ...

// Finish — derive the auth key from the completed handshake
let done = auth::finish(state3, dh_answer)?;

// done.auth_key    → [u8; 256]   — the shared secret
// done.first_salt  → i64         — first server salt to use
// done.time_offset → i32         — clock skew relative to server
```

---

### `Message`

A decoded MTProto message as returned by `session.unpack()`:

```rust
pub struct Message {
    pub msg_id:  i64,
    pub seq_no:  i32,
    pub salt:    i64,
    pub body:    Vec<u8>,   // raw TL bytes of the inner object
}
```

The `body` bytes are deserialized by `layer-client` using `layer-tl-types`'s `Deserializable` trait.

---

### `Session` (plain / unencrypted)

Used only for sending the initial DH handshake messages before an auth key exists:

```rust
use layer_mtproto::Session;

let mut plain = Session::new();
// Pack a plaintext MTProto message
let framed = plain.pack_plain(&my_handshake_request)?;
```

---

## 🔍 What's Inside

### Encryption Details

- AES-IGE encryption/decryption via `layer-crypto`
- `msg_key` = SHA-256 of `(auth_key[88..120] || plaintext)` for client→server
- `msg_key` = SHA-256 of `(auth_key[96..128] || plaintext)` for server→client
- The 256-byte auth key is split into sub-keys for encryption, MAC, and padding

### Message Framing

Every outgoing MTProto message has this structure:

```
server_salt     (8 bytes) — current server salt
session_id      (8 bytes) — random, stable for this session's lifetime
message_id      (8 bytes) — Unix time * 2^32, monotonically increasing
seq_no          (4 bytes) — content-related message counter
message_length  (4 bytes) — length of payload in bytes
payload         (N bytes) — serialized TL object
padding         (M bytes) — 12–1024 random bytes so total % 16 == 0
```

Wrapped in a 32-byte `msg_key` prefix after encryption.

### Containers & Compression

- `msg_container` (TL ID `0x73f1f8dc`) — wraps multiple logical messages in one TCP frame; both packing and unpacking are supported
- `gzip_packed` (TL ID `0x3072cfa1`) — response bodies are decompressed automatically; outgoing large requests are optionally compressed

### Salt & Time Management

- `bad_server_salt` — session records the corrected salt and automatically resets
- `future_salts` prefetch — salts are requested in advance; session rotates before expiry
- `time_offset` correction — all outgoing `msg_id` values are adjusted for clock skew

### Error Recovery

- `bad_msg_notification` — messages with wrong `msg_id`, `seq_no`, or `session_id` are resent with corrected framing
- `seq_no` auto-correction for error codes 32 (seq_no too low) and 33 (seq_no too high)
- `msg_resend_req` — fulfils resend requests by replaying from a sent-body cache

### Acknowledgements

- Received content-related messages accumulate in a `pending_ack` list
- The `flush_acks()` call bundles them into a single `MsgsAck` message
- `layer-client` flushes pending ACKs on a timer and before each outgoing call

---

## 🔗 Part of the layer stack

```
layer-client
└── layer-mtproto     ← you are here
    ├── layer-tl-types  (tl-mtproto feature)
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
