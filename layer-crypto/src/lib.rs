//! Cryptographic primitives for Telegram MTProto.
//!
//! Provides:
//! - AES-256-IGE encryption/decryption
//! - SHA-1 / SHA-256 hash macros
//! - Pollard-rho PQ factorization
//! - RSA padding (MTProto RSA-PAD scheme)
//! - `AuthKey` — 256-byte session key
//! - MTProto 2.0 message encryption / decryption
//! - DH nonce→key derivation

#![deny(unsafe_code)]

pub mod aes;
mod auth_key;
mod deque_buffer;
mod factorize;
pub mod rsa;
mod sha;

pub use auth_key::AuthKey;
pub use deque_buffer::DequeBuffer;
pub use factorize::factorize;

// ─── MTProto 2.0 encrypt / decrypt ───────────────────────────────────────────

/// Errors from [`decrypt_data_v2`].
#[derive(Clone, Debug, PartialEq)]
pub enum DecryptError {
    /// Ciphertext too short or not block-aligned.
    InvalidBuffer,
    /// The `auth_key_id` in the ciphertext does not match our key.
    AuthKeyMismatch,
    /// The `msg_key` in the ciphertext does not match our computed value.
    MessageKeyMismatch,
}

impl std::fmt::Display for DecryptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBuffer => write!(f, "invalid ciphertext buffer length"),
            Self::AuthKeyMismatch => write!(f, "auth_key_id mismatch"),
            Self::MessageKeyMismatch => write!(f, "msg_key mismatch"),
        }
    }
}
impl std::error::Error for DecryptError {}

enum Side { Client, Server }
impl Side {
    fn x(&self) -> usize { match self { Side::Client => 0, Side::Server => 8 } }
}

fn calc_key(auth_key: &AuthKey, msg_key: &[u8; 16], side: Side) -> ([u8; 32], [u8; 32]) {
    let x = side.x();
    let sha_a = sha256!(msg_key, &auth_key.data[x..x + 36]);
    let sha_b = sha256!(&auth_key.data[40 + x..40 + x + 36], msg_key);

    let mut aes_key = [0u8; 32];
    aes_key[..8].copy_from_slice(&sha_a[..8]);
    aes_key[8..24].copy_from_slice(&sha_b[8..24]);
    aes_key[24..].copy_from_slice(&sha_a[24..]);

    let mut aes_iv = [0u8; 32];
    aes_iv[..8].copy_from_slice(&sha_b[..8]);
    aes_iv[8..24].copy_from_slice(&sha_a[8..24]);
    aes_iv[24..].copy_from_slice(&sha_b[24..]);

    (aes_key, aes_iv)
}

fn padding_len(len: usize) -> usize {
    16 + (16 - (len % 16))
}

/// Encrypt `buffer` (in-place, with prepended header) using MTProto 2.0.
///
/// After this call `buffer` contains `key_id || msg_key || ciphertext`.
pub fn encrypt_data_v2(buffer: &mut DequeBuffer, auth_key: &AuthKey) {
    let mut rnd = [0u8; 32];
    getrandom::getrandom(&mut rnd).expect("getrandom failed");
    do_encrypt_data_v2(buffer, auth_key, &rnd);
}

pub(crate) fn do_encrypt_data_v2(buffer: &mut DequeBuffer, auth_key: &AuthKey, rnd: &[u8; 32]) {
    let pad = padding_len(buffer.len());
    buffer.extend(rnd.iter().take(pad).copied());

    let x = Side::Client.x();
    let msg_key_large = sha256!(&auth_key.data[88 + x..88 + x + 32], buffer.as_ref());
    let mut msg_key = [0u8; 16];
    msg_key.copy_from_slice(&msg_key_large[8..24]);

    let (key, iv) = calc_key(auth_key, &msg_key, Side::Client);
    aes::ige_encrypt(buffer.as_mut(), &key, &iv);

    buffer.extend_front(&msg_key);
    buffer.extend_front(&auth_key.key_id);
}

/// Decrypt an MTProto 2.0 ciphertext.
///
/// `buffer` must start with `key_id || msg_key || ciphertext`.
/// On success returns a slice of `buffer` containing the plaintext.
pub fn decrypt_data_v2<'a>(buffer: &'a mut [u8], auth_key: &AuthKey) -> Result<&'a mut [u8], DecryptError> {
    if buffer.len() < 24 || (buffer.len() - 24) % 16 != 0 {
        return Err(DecryptError::InvalidBuffer);
    }
    if auth_key.key_id != buffer[..8] {
        return Err(DecryptError::AuthKeyMismatch);
    }
    let mut msg_key = [0u8; 16];
    msg_key.copy_from_slice(&buffer[8..24]);

    let (key, iv) = calc_key(auth_key, &msg_key, Side::Server);
    aes::ige_decrypt(&mut buffer[24..], &key, &iv);

    let x = Side::Server.x();
    let our_key = sha256!(&auth_key.data[88 + x..88 + x + 32], &buffer[24..]);
    if msg_key != our_key[8..24] {
        return Err(DecryptError::MessageKeyMismatch);
    }
    Ok(&mut buffer[24..])
}

/// Derive `(key, iv)` from nonces for decrypting `ServerDhParams.encrypted_answer`.
pub fn generate_key_data_from_nonce(server_nonce: &[u8; 16], new_nonce: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
    let h1 = sha1!(new_nonce, server_nonce);
    let h2 = sha1!(server_nonce, new_nonce);
    let h3 = sha1!(new_nonce, new_nonce);

    let mut key = [0u8; 32];
    key[..20].copy_from_slice(&h1);
    key[20..].copy_from_slice(&h2[..12]);

    let mut iv = [0u8; 32];
    iv[..8].copy_from_slice(&h2[12..]);
    iv[8..28].copy_from_slice(&h3);
    iv[28..].copy_from_slice(&new_nonce[..4]);

    (key, iv)
}
