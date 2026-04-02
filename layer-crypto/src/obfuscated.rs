//! AES-256-CTR cipher for MTProto transport obfuscation.
//!
//! Telegram's obfuscated transport encrypts the entire byte stream with two
//! separate AES-256-CTR instances (TX and RX) whose keys are derived from a
//! random 64-byte init header sent at connection start.
//!
//! Key derivation from the 64-byte `init` buffer:
//! ```text
//! TX key = init[8..40]           TX IV = init[40..56]
//! RX key = reverse(init)[8..40]  RX IV = reverse(init)[40..56]
//! ```

#[allow(deprecated)]
use aes::cipher::{KeyIvInit, StreamCipher, generic_array::GenericArray};

/// AES-256-CTR stream cipher pair for MTProto obfuscated transport.
pub struct ObfuscatedCipher {
    #[allow(deprecated)]
    rx: ctr::Ctr128BE<aes::Aes256>,
    #[allow(deprecated)]
    tx: ctr::Ctr128BE<aes::Aes256>,
}

impl ObfuscatedCipher {
    /// Build cipher state from the 64-byte random init buffer.
    #[allow(deprecated)]
    pub fn new(init: &[u8; 64]) -> Self {
        let rev: Vec<u8> = init.iter().copied().rev().collect();
        Self {
            rx: ctr::Ctr128BE::<aes::Aes256>::new(
                GenericArray::from_slice(&rev[8..40]),
                GenericArray::from_slice(&rev[40..56]),
            ),
            tx: ctr::Ctr128BE::<aes::Aes256>::new(
                GenericArray::from_slice(&init[8..40]),
                GenericArray::from_slice(&init[40..56]),
            ),
        }
    }

    /// Encrypt outgoing bytes in-place (TX direction).
    pub fn encrypt(&mut self, buf: &mut [u8]) {
        self.tx.apply_keystream(buf);
    }

    /// Decrypt incoming bytes in-place (RX direction).
    pub fn decrypt(&mut self, buf: &mut [u8]) {
        self.rx.apply_keystream(buf);
    }
}
