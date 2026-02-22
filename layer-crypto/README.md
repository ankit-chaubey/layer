<div align="center">

# 🔐 layer-crypto

**Cryptographic primitives for the Telegram MTProto 2.0 protocol.**

[![Crates.io](https://img.shields.io/crates/v/layer-crypto?color=fc8d62)](https://crates.io/crates/layer-crypto)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-2024_edition-f74c00)](https://www.rust-lang.org/)

*AES-IGE, RSA, SHA, DH — everything MTProto needs to secure a connection.*

</div>

---

## 📦 Installation

```toml
[dependencies]
layer-crypto = "0.1.1"
```

---

## ✨ What It Does

`layer-crypto` implements all the cryptographic operations required by the Telegram MTProto 2.0 protocol — from the initial RSA-encrypted DH handshake all the way to the per-message AES-IGE encryption. Every algorithm here is implemented from scratch to match Telegram's exact specification.

---

## 🔧 What's Inside

### AES-IGE (`aes.rs`)

MTProto uses **AES-IGE** (Infinite Garble Extension) mode — not a standard mode you'll find in most crypto libraries. Implemented from scratch.

```rust
use layer_crypto::aes::{ige_encrypt, ige_decrypt};

// key: 32 bytes, iv: 32 bytes
let ciphertext = ige_encrypt(&plaintext, &key, &iv);
let plaintext  = ige_decrypt(&ciphertext, &key, &iv);
```

### RSA (`rsa.rs`)

Used during the DH handshake to encrypt the `p_q_inner_data` with Telegram's server public key.

```rust
use layer_crypto::rsa::encrypt;

let encrypted = encrypt(&data, &public_key_modulus, &public_key_exponent);
```

### SHA (`sha.rs`)

Both SHA-1 (used in auth key derivation and older message signatures) and SHA-256 (used in MTProto 2.0 `msg_key` derivation).

```rust
use layer_crypto::sha::{sha1, sha256};

let hash1 = sha1(&data);
let hash2 = sha256(&data);
```

### Auth Key Derivation

After the DH key exchange, the raw shared secret `g^(a*b) mod p` is expanded into the 2048-bit auth key using a specific SHA-1-based KDF defined by Telegram.

### PQ Factorization (`factorize.rs`)

During `step1` of the handshake, the server sends a `pq` value that the client must factor into `p` and `q`. Uses **Pollard's rho algorithm** for fast factorization.

```rust
use layer_crypto::factorize::factorize;

let (p, q) = factorize(pq);
```

### Diffie-Hellman

The `g^a mod p` and shared secret computations use big-integer arithmetic via `num-bigint`.

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
