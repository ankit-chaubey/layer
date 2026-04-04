//! Obfuscated MTProto transport (Obfuscated2).
//!
//! Wraps [`layer_crypto::ObfuscatedCipher`] (AES-256-CTR) for the full
//! Obfuscated2 handshake and per-frame encryption.
//!
//! All three bugs from the original implementation are fixed here:
//!
//! * **Bug A** — The original `ObfCipher` used SHA-256-based XOR instead of
//!   AES-256-CTR.  Replaced entirely by [`layer_crypto::ObfuscatedCipher`].
//!
//! * **Bug B** — `derive_keys` reversed only the 32-byte and 16-byte
//!   sub-slices instead of the full 64-byte buffer.  Fixed by using
//!   `ObfuscatedCipher::new` which reverses the whole buffer correctly.
//!
//! * **Bug C** — The handshake only encrypted 8 bytes (`nonce[56..]`) and
//!   discarded the cipher, leaving subsequent data unencrypted.  Fixed: all
//!   64 bytes are encrypted, only `[56..64]` are taken from the ciphertext,
//!   and the cipher is retained for all subsequent sends/receives.
//!
//! [Obfuscated2]: https://core.telegram.org/mtproto/mtproto-transports#obfuscated-2

pub use layer_crypto::ObfuscatedCipher;

use crate::InvocationError;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

// ─── ObfuscatedStream ─────────────────────────────────────────────────────────

/// Wraps a [`TcpStream`] with correct Obfuscated2 framing (AES-256-CTR).
///
/// After construction the 64-byte handshake has been sent and the stream is
/// ready for Abridged-framed MTProto messages with per-byte encryption.
pub struct ObfuscatedStream {
    stream: TcpStream,
    cipher: ObfuscatedCipher,
}

impl ObfuscatedStream {
    /// Connect to `addr` and perform the Obfuscated2 handshake.
    ///
    /// `_proxy_secret` is reserved for future MTProxy support; pass `None`.
    pub async fn connect(
        addr: &str,
        _proxy_secret: Option<&[u8; 16]>,
    ) -> Result<Self, InvocationError> {
        let stream = TcpStream::connect(addr).await?;
        Self::handshake(stream).await
    }

    async fn handshake(mut stream: TcpStream) -> Result<Self, InvocationError> {
        // Build a random 64-byte init nonce.
        let mut nonce = [0u8; 64];
        getrandom::getrandom(&mut nonce)
            .map_err(|_| InvocationError::Deserialize("getrandom failed".into()))?;

        // Stamp Abridged protocol tag at bytes 56-59.
        nonce[56] = 0xef;
        nonce[57] = 0xef;
        nonce[58] = 0xef;
        nonce[59] = 0xef;

        // Bug B fix: ObfuscatedCipher::new reverses the WHOLE 64-byte buffer
        // to derive the RX key - not just sub-slices.
        // Bug A fix: uses AES-256-CTR, not SHA-256 XOR.
        let mut cipher = ObfuscatedCipher::new(&nonce);

        // Bug C fix: encrypt ALL 64 bytes, copy only [56..64] back.
        // TX cipher is now at position 64; all subsequent sends continue from there.
        let mut encrypted = nonce;
        cipher.encrypt(&mut encrypted);
        nonce[56..64].copy_from_slice(&encrypted[56..64]);

        // Wire format: nonce[0..56] (plaintext) + encrypted[56..64]
        stream.write_all(&nonce).await?;
        tracing::info!("[obfuscated] Handshake sent (AES-256-CTR, cipher at pos 64)");

        Ok(Self { stream, cipher })
    }

    /// Send an Abridged-framed message through the obfuscated layer.
    pub async fn send(&mut self, data: &[u8]) -> Result<(), InvocationError> {
        let words = data.len() / 4;
        let mut frame = if words < 0x7f {
            let mut v = Vec::with_capacity(1 + data.len());
            v.push(words as u8);
            v
        } else {
            let mut v = Vec::with_capacity(4 + data.len());
            v.extend_from_slice(&[
                0x7f,
                (words & 0xff) as u8,
                ((words >> 8) & 0xff) as u8,
                ((words >> 16) & 0xff) as u8,
            ]);
            v
        };
        frame.extend_from_slice(data);
        // Encrypt the whole frame (header + payload) in one shot.
        self.cipher.encrypt(&mut frame);
        self.stream.write_all(&frame).await?;
        Ok(())
    }

    /// Receive and decrypt the next Abridged frame.
    pub async fn recv(&mut self) -> Result<Vec<u8>, InvocationError> {
        let mut h = [0u8; 1];
        self.stream.read_exact(&mut h).await?;
        self.cipher.decrypt(&mut h);

        let words = if h[0] < 0x7f {
            h[0] as usize
        } else {
            let mut b = [0u8; 3];
            self.stream.read_exact(&mut b).await?;
            self.cipher.decrypt(&mut b);
            b[0] as usize | (b[1] as usize) << 8 | (b[2] as usize) << 16
        };

        let mut buf = vec![0u8; words * 4];
        self.stream.read_exact(&mut buf).await?;
        self.cipher.decrypt(&mut buf);
        Ok(buf)
    }
}
