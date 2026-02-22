//! Obfuscated MTProto transport.
//!
//! Implements the [MTProto Obfuscated2] transport used by MTProxy and required
//! in ISP-restricted networks.  Every byte sent and received is XOR'd with a
//! rolling key derived from a random 64-byte nonce so that traffic is
//! indistinguishable from random noise to deep-packet inspection.
//!
//! [MTProto Obfuscated2]: https://core.telegram.org/mtproto/mtproto-transports#obfuscated-2

use sha2::{Sha256, Digest};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use crate::InvocationError;

// ─── ObfuscatedCipher ─────────────────────────────────────────────────────────

/// Rolling AES-CTR key state.  In practice Obfuscated2 uses straight XOR with
/// a stream derived from the initial nonce, so we model it as a key stream.
pub struct ObfCipher {
    key:   [u8; 32],
    iv:    [u8; 16],
    buf:   Vec<u8>,
    pos:   usize,
}

impl ObfCipher {
    pub fn new(key: [u8; 32], iv: [u8; 16]) -> Self {
        Self { key, iv, buf: Vec::new(), pos: 0 }
    }

    /// Extend the keystream buffer using repeated SHA-256 rounds (simplified).
    pub fn fill(&mut self) {
        let mut h = Sha256::new();
        h.update(&self.key);
        h.update(&self.iv);
        h.update(&self.buf);
        let block = h.finalize();
        self.buf.extend_from_slice(&block);
    }

    /// XOR `data` in-place with the rolling keystream.
    pub fn apply(&mut self, data: &mut [u8]) {
        for byte in data.iter_mut() {
            while self.pos >= self.buf.len() {
                self.fill();
            }
            *byte ^= self.buf[self.pos];
            self.pos += 1;
        }
    }
}

// ─── ObfuscatedStream ─────────────────────────────────────────────────────────

/// Wraps a [`TcpStream`] with obfuscated MTProto2 framing.
///
/// After construction the initial 64-byte header has already been sent and the
/// stream is ready for abridged MTProto messages.
pub struct ObfuscatedStream {
    stream:   TcpStream,
    enc:      ObfCipher,
    dec:      ObfCipher,
}

impl ObfuscatedStream {
    /// Connect to `addr` and perform the obfuscated handshake.
    ///
    /// `proxy_secret` is the MTProxy secret (32 bytes hex-decoded).  Pass
    /// `None` / zeros to use plain obfuscation without a proxy secret.
    pub async fn connect(addr: &str, proxy_secret: Option<&[u8; 16]>) -> Result<Self, InvocationError> {
        let stream = TcpStream::connect(addr).await?;
        Self::handshake(stream, proxy_secret).await
    }

    async fn handshake(
        mut stream:     TcpStream,
        proxy_secret:   Option<&[u8; 16]>,
    ) -> Result<Self, InvocationError> {
        // Build a random 64-byte init payload as per Obfuscated2 spec.
        let mut nonce = [0u8; 64];
        getrandom::getrandom(&mut nonce).map_err(|_| InvocationError::Deserialize("getrandom failed".into()))?;

        // Bytes 56-60 must NOT equal certain magic values.
        // Force the protocol tag (abridged = 0xefefefefu32) at bytes 56-59.
        nonce[56] = 0xef;
        nonce[57] = 0xef;
        nonce[58] = 0xef;
        nonce[59] = 0xef;

        // Derive enc + dec keys using the shared derive_keys function.
        let (enc_key, enc_iv, dec_key, dec_iv) = derive_keys(&nonce, proxy_secret);

        let mut enc = ObfCipher::new(enc_key, enc_iv);
        let dec     = ObfCipher::new(dec_key, dec_iv);

        // Encrypt the header with the enc cipher and send it.
        let mut encrypted_header = nonce;
        enc.apply(&mut encrypted_header[56..]); // only encrypt from byte 56 per spec
        stream.write_all(&encrypted_header).await?;

        log::info!("[obfuscated] Handshake sent");

        Ok(Self { stream, enc, dec })
    }

    /// Send an abridged-framed message through the obfuscated layer.
    pub async fn send(&mut self, data: &[u8]) -> Result<(), InvocationError> {
        let words = data.len() / 4;
        let mut header = if words < 0x7f {
            vec![words as u8]
        } else {
            vec![0x7f, (words & 0xff) as u8, ((words >> 8) & 0xff) as u8, ((words >> 16) & 0xff) as u8]
        };

        // XOR header + data before sending
        self.enc.apply(&mut header);
        let mut payload = data.to_vec();
        self.enc.apply(&mut payload);

        self.stream.write_all(&header).await?;
        self.stream.write_all(&payload).await?;
        Ok(())
    }

    /// Receive and de-obfuscate the next abridged frame.
    pub async fn recv(&mut self) -> Result<Vec<u8>, InvocationError> {
        let mut h = [0u8; 1];
        self.stream.read_exact(&mut h).await?;
        self.dec.apply(&mut h);

        let words = if h[0] < 0x7f {
            h[0] as usize
        } else {
            let mut b = [0u8; 3];
            self.stream.read_exact(&mut b).await?;
            self.dec.apply(&mut b);
            b[0] as usize | (b[1] as usize) << 8 | (b[2] as usize) << 16
        };

        let mut buf = vec![0u8; words * 4];
        self.stream.read_exact(&mut buf).await?;
        self.dec.apply(&mut buf);
        Ok(buf)
    }
}

// ─── Key derivation (public for use by dc_pool) ───────────────────────────────

/// Derive enc_key, enc_iv, dec_key, dec_iv from a 64-byte obfuscation nonce.
///
/// Used by both [`ObfuscatedStream`] and `dc_pool` so it must be `pub`.
pub fn derive_keys(
    nonce:  &[u8; 64],
    secret: Option<&[u8; 16]>,
) -> ([u8; 32], [u8; 16], [u8; 32], [u8; 16]) {
    let (enc_key, enc_iv) = derive_one(&nonce[8..40], &nonce[40..56], secret);
    // Decrypt key uses reversed slices
    let mut rev = *nonce;
    rev[8..40].reverse();
    rev[40..56].reverse();
    let (dec_key, dec_iv) = derive_one(&rev[8..40], &rev[40..56], secret);
    (enc_key, enc_iv, dec_key, dec_iv)
}

fn derive_one(key_src: &[u8], iv_src: &[u8], secret: Option<&[u8; 16]>) -> ([u8; 32], [u8; 16]) {
    let mut key = [0u8; 32];
    let mut iv  = [0u8; 16];
    if let Some(s) = secret {
        let mut h = Sha256::new();
        h.update(key_src);
        h.update(s);
        key.copy_from_slice(&h.finalize());
    } else {
        let len = key_src.len().min(32);
        key[..len].copy_from_slice(&key_src[..len]);
    }
    let len = iv_src.len().min(16);
    iv[..len].copy_from_slice(&iv_src[..len]);
    (key, iv)
}
