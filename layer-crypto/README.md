<div align="center">

<img src="https://raw.githubusercontent.com/ankit-chaubey/layer/main/docs/images/crate-crypto-banner.svg" alt="layer-crypto" width="100%" />

# 🔐 layer-crypto

**Cryptographic primitives for the Telegram MTProto 2.0 protocol.**

[![Crates.io](https://img.shields.io/crates/v/layer-crypto?color=fc8d62)](https://crates.io/crates/layer-crypto)
[![docs.rs](https://img.shields.io/badge/docs.rs-layer--crypto-5865F2)](https://docs.rs/layer-crypto)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-2024_edition-f74c00)](https://www.rust-lang.org/)

*AES-IGE, RSA, SHA, DH — everything MTProto needs to secure a connection.*

</div>

---

## 📦 Installation

```toml
[dependencies]
layer-crypto = "0.4.5"
```

---

## ✨ What It Does

`layer-crypto` implements all the cryptographic operations required by the Telegram MTProto 2.0 protocol — from the initial RSA-encrypted DH handshake all the way to the per-message AES-IGE encryption and the transport-layer obfuscation init.

Every algorithm here is implemented from scratch to match Telegram's exact specification. No external Telegram-specific crypto libraries are used.

---

## 🔧 What's Inside

### AES-IGE (`aes.rs`)

MTProto uses **AES-IGE** (Infinite Garble Extension) mode — not a standard mode you'll find in most crypto libraries. Implemented from scratch over the `aes` crate's block cipher.

```rust
use layer_crypto::aes::{ige_encrypt, ige_decrypt};

// key: 32 bytes, iv: 32 bytes
let ciphertext = ige_encrypt(&plaintext, &key, &iv);
let recovered  = ige_decrypt(&ciphertext, &key, &iv);
assert_eq!(plaintext, recovered);
```

Used by `layer-mtproto` for encrypting every outgoing MTProto message and decrypting every incoming one.

---

### RSA (`rsa.rs`)

Used during the DH handshake to encrypt `p_q_inner_data` with Telegram's server public key. Operates with `num-bigint` for arbitrary-precision modular exponentiation.

```rust
use layer_crypto::rsa::encrypt;

// data: ≤ 255 bytes, modulus and exponent from Telegram's public key
let encrypted = encrypt(&data, &public_key_modulus, &public_key_exponent);
```

---

### SHA (`sha.rs`)

Provides both SHA-1 and SHA-256:

- **SHA-1** — used in auth key derivation fingerprinting and in the older `msg_key` derivation path.
- **SHA-256** — used in MTProto 2.0 `msg_key` derivation (the variant used for all modern sessions).

```rust
use layer_crypto::sha::{sha1, sha256};

let hash1 = sha1(&data);   // [u8; 20]
let hash2 = sha256(&data); // [u8; 32]
```

---

### Auth Key Derivation

After the DH exchange, the raw shared secret `g^(ab) mod p` is expanded into the final 2048-bit auth key using a SHA-1-based KDF defined by Telegram's spec. This runs inside `layer-mtproto`'s `authentication::finish()`.

The KDF uses a specific sequence of SHA-1 hashes over different slices of the DH secret to derive exactly 256 bytes of key material.

---

### PQ Factorization (`factorize.rs`)

During Step 1 of the DH handshake, the server sends a product `pq` that the client must factor into its two prime factors `p` and `q`. This is deliberate — it's a proof-of-work that limits spam connections.

Uses **Pollard's rho algorithm** for fast factorization (O(n^¼) expected time vs O(√n) for trial division):

```rust
use layer_crypto::factorize::factorize;

let pq: u64 = 0x17ED48941A08F981;
let (p, q) = factorize(pq);
// p * q == pq, p < q, both prime
```

---

### Diffie-Hellman

The `g^a mod p` and shared-secret `g^(ab) mod p` computations use big-integer arithmetic via `num-bigint`. The DH parameters `g` (generator) and `p` (safe prime) are received from the server and validated before use.

---

### Transport Obfuscation (`obfuscated.rs`)

MTProto supports an **obfuscated transport** where all bytes — including the TCP handshake — are XOR-encrypted to resist protocol fingerprinting by middleboxes.

`layer-crypto` provides the stateful `ObfuscatedCodec` used by `layer-client`'s obfuscated transport variant:

- Generates a random 64-byte init payload
- Derives encrypt/decrypt AES-CTR keys from the init payload
- Streams the XOR transformation over the raw TCP bytes

```rust
use layer_crypto::obfuscated::ObfuscatedCodec;

let (codec, init_bytes) = ObfuscatedCodec::new()?;
// send init_bytes to server first, then use codec for all subsequent I/O
```

---

### Deque Buffer (`deque_buffer.rs`)

A `VecDeque`-backed byte buffer used internally for streaming I/O without unnecessary allocations. Supports efficient `push_back` / `drain_front` patterns used by the transport layer.

---

## 🔒 Security Note

This library is purpose-built for the Telegram MTProto protocol. The algorithms are implemented to match Telegram's exact specification, not for general-purpose cryptographic use. If you need general crypto in Rust, use the [RustCrypto](https://github.com/RustCrypto) crates.

---

## 🔗 Part of the layer stack

```
layer-client
└── layer-mtproto
    ├── layer-tl-types
    └── layer-crypto    ← you are here
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
