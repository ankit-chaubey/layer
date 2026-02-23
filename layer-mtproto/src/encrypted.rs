//! Encrypted MTProto 2.0 session (post auth-key).
//!
//! Once you have a `Finished` from [`crate::authentication`], construct an
//! [`EncryptedSession`] and use it to serialize/deserialize all subsequent
//! messages.

use std::time::{SystemTime, UNIX_EPOCH};

use layer_crypto::{AuthKey, DequeBuffer, decrypt_data_v2, encrypt_data_v2};
use layer_tl_types::RemoteCall;


/// Errors that can occur when decrypting a server message.
#[derive(Debug)]
pub enum DecryptError {
    /// The underlying crypto layer rejected the message.
    Crypto(layer_crypto::DecryptError),
    /// The decrypted inner message was too short to contain a valid header.
    FrameTooShort,
    /// Session-ID mismatch (possible replay or wrong connection).
    SessionMismatch,
}

impl std::fmt::Display for DecryptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Crypto(e) => write!(f, "crypto: {e}"),
            Self::FrameTooShort => write!(f, "inner plaintext too short"),
            Self::SessionMismatch => write!(f, "session_id mismatch"),
        }
    }
}
impl std::error::Error for DecryptError {}

/// The inner payload extracted from a successfully decrypted server frame.
pub struct DecryptedMessage {
    /// `salt` sent by the server.
    pub salt:       i64,
    /// The `session_id` from the frame.
    pub session_id: i64,
    /// The `msg_id` of the inner message.
    pub msg_id:     i64,
    /// `seq_no` of the inner message.
    pub seq_no:     i32,
    /// TL-serialized body of the inner message.
    pub body:       Vec<u8>,
}

/// MTProto 2.0 encrypted session state.
///
/// Wraps an `AuthKey` and tracks per-session counters (session_id, seq_no,
/// last_msg_id, server salt).  Use [`EncryptedSession::pack`] to encrypt
/// outgoing requests and [`EncryptedSession::unpack`] to decrypt incoming
/// server frames.
pub struct EncryptedSession {
    auth_key:    AuthKey,
    session_id:  i64,
    sequence:    i32,
    last_msg_id: i64,
    /// Current server salt to include in outgoing messages.
    pub salt:    i64,
    /// Clock skew in seconds vs. server.
    pub time_offset: i32,
}

impl EncryptedSession {
    /// Create a new encrypted session from the output of `authentication::finish`.
    pub fn new(auth_key: [u8; 256], first_salt: i64, time_offset: i32) -> Self {
        let mut rnd = [0u8; 8];
        getrandom::getrandom(&mut rnd).expect("getrandom");
        Self {
            auth_key: AuthKey::from_bytes(auth_key),
            session_id: i64::from_le_bytes(rnd),
            sequence: 0,
            last_msg_id: 0,
            salt: first_salt,
            time_offset,
        }
    }

    /// Compute the next message ID (based on corrected server time).
    fn next_msg_id(&mut self) -> i64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH).unwrap();
        let secs = (now.as_secs() as i32).wrapping_add(self.time_offset) as u64;
        let nanos = now.subsec_nanos() as u64;
        let mut id = ((secs << 32) | (nanos << 2)) as i64;
        if self.last_msg_id >= id { id = self.last_msg_id + 4; }
        self.last_msg_id = id;
        id
    }

    /// Next content-related seq_no (odd) and advance the counter.
    fn next_seq_no(&mut self) -> i32 {
        let n = self.sequence * 2 + 1;
        self.sequence += 1;
        n
    }

    /// Serialize and encrypt a TL function into a wire-ready byte vector.
    ///
    /// Layout of the plaintext before encryption:
    /// ```text
    /// salt:       i64
    /// session_id: i64
    /// msg_id:     i64
    /// seq_no:     i32
    /// body_len:   i32
    /// body:       [u8; body_len]
    /// ```
    /// Like `pack` but only requires `Serializable` (not `RemoteCall`).
    /// Useful for generic wrapper types like `InvokeWithLayer<InitConnection<X>>`
    /// where the return type is determined by the inner call, not the wrapper.
    pub fn pack_serializable<S: layer_tl_types::Serializable>(&mut self, call: &S) -> Vec<u8> {
        let body = call.to_bytes();
        let msg_id = self.next_msg_id();
        let seq_no = self.next_seq_no();

        let inner_len = 8 + 8 + 8 + 4 + 4 + body.len();
        let mut buf = DequeBuffer::with_capacity(inner_len, 32);
        buf.extend(self.salt.to_le_bytes());
        buf.extend(self.session_id.to_le_bytes());
        buf.extend(msg_id.to_le_bytes());
        buf.extend(seq_no.to_le_bytes());
        buf.extend((body.len() as u32).to_le_bytes());
        buf.extend(body.iter().copied());

        encrypt_data_v2(&mut buf, &self.auth_key);
        buf.as_ref().to_vec()
    }


    /// Like [`pack_serializable`] but also returns the `msg_id`.
    /// Used by the split-writer path for write RPCs (Serializable but not RemoteCall).
    pub fn pack_serializable_with_msg_id<S: layer_tl_types::Serializable>(&mut self, call: &S) -> (Vec<u8>, i64) {
        let body   = call.to_bytes();
        let msg_id = self.next_msg_id();
        let seq_no = self.next_seq_no();
        let inner_len = 8 + 8 + 8 + 4 + 4 + body.len();
        let mut buf   = DequeBuffer::with_capacity(inner_len, 32);
        buf.extend(self.salt.to_le_bytes());
        buf.extend(self.session_id.to_le_bytes());
        buf.extend(msg_id.to_le_bytes());
        buf.extend(seq_no.to_le_bytes());
        buf.extend((body.len() as u32).to_le_bytes());
        buf.extend(body.iter().copied());
        encrypt_data_v2(&mut buf, &self.auth_key);
        (buf.as_ref().to_vec(), msg_id)
    }

    /// Like [`pack`] but also returns the `msg_id` allocated for this message.
    ///
    /// Used by the async client to register a pending RPC reply channel keyed
    /// by `msg_id` *before* sending the packet.
    pub fn pack_with_msg_id<R: RemoteCall>(&mut self, call: &R) -> (Vec<u8>, i64) {
        let body   = call.to_bytes();
        let msg_id = self.next_msg_id();
        let seq_no = self.next_seq_no();
        let inner_len = 8 + 8 + 8 + 4 + 4 + body.len();
        let mut buf   = DequeBuffer::with_capacity(inner_len, 32);
        buf.extend(self.salt.to_le_bytes());
        buf.extend(self.session_id.to_le_bytes());
        buf.extend(msg_id.to_le_bytes());
        buf.extend(seq_no.to_le_bytes());
        buf.extend((body.len() as u32).to_le_bytes());
        buf.extend(body.iter().copied());
        encrypt_data_v2(&mut buf, &self.auth_key);
        (buf.as_ref().to_vec(), msg_id)
    }

    /// Encrypt and frame a [`RemoteCall`] into a ready-to-send MTProto message.
    ///
    /// Returns the encrypted bytes to pass directly to the transport layer.
    pub fn pack<R: RemoteCall>(&mut self, call: &R) -> Vec<u8> {
        let body = call.to_bytes();
        let msg_id = self.next_msg_id();
        let seq_no = self.next_seq_no();

        // Build plaintext inner payload
        let inner_len = 8 + 8 + 8 + 4 + 4 + body.len();
        // Front capacity = 32 for auth_key_id + msg_key
        let mut buf = DequeBuffer::with_capacity(inner_len, 32);
        buf.extend(self.salt.to_le_bytes());
        buf.extend(self.session_id.to_le_bytes());
        buf.extend(msg_id.to_le_bytes());
        buf.extend(seq_no.to_le_bytes());
        buf.extend((body.len() as u32).to_le_bytes());
        buf.extend(body.iter().copied());

        encrypt_data_v2(&mut buf, &self.auth_key);
        buf.as_ref().to_vec()
    }

    /// Decrypt an encrypted server frame.
    ///
    /// `frame` should be a raw frame received from the transport (already
    /// stripped of the abridged-length prefix).
    pub fn unpack(&self, frame: &mut Vec<u8>) -> Result<DecryptedMessage, DecryptError> {
        let plaintext = decrypt_data_v2(frame, &self.auth_key)
            .map_err(DecryptError::Crypto)?;

        // inner: salt(8) + session_id(8) + msg_id(8) + seq_no(4) + len(4) + body
        if plaintext.len() < 32 {
            return Err(DecryptError::FrameTooShort);
        }

        let salt       = i64::from_le_bytes(plaintext[..8].try_into().unwrap());
        let session_id = i64::from_le_bytes(plaintext[8..16].try_into().unwrap());
        let msg_id     = i64::from_le_bytes(plaintext[16..24].try_into().unwrap());
        let seq_no     = i32::from_le_bytes(plaintext[24..28].try_into().unwrap());
        let body_len   = u32::from_le_bytes(plaintext[28..32].try_into().unwrap()) as usize;

        if session_id != self.session_id {
            return Err(DecryptError::SessionMismatch);
        }

        let body = plaintext[32..32 + body_len.min(plaintext.len() - 32)].to_vec();

        Ok(DecryptedMessage { salt, session_id, msg_id, seq_no, body })
    }

    /// Return the auth_key bytes (for persistence).
    pub fn auth_key_bytes(&self) -> [u8; 256] { self.auth_key.to_bytes() }

    /// Return the current session_id.
    pub fn session_id(&self) -> i64 { self.session_id }
}

impl EncryptedSession {
    /// Decrypt a frame using explicit key + session_id — no mutable state needed.
    /// Used by the split-reader task so it can decrypt without locking the writer.
    pub fn decrypt_frame(
        auth_key:   &[u8; 256],
        session_id: i64,
        frame:      &mut Vec<u8>,
    ) -> Result<DecryptedMessage, DecryptError> {
        let key = AuthKey::from_bytes(*auth_key);
        let plaintext = decrypt_data_v2(frame, &key)
            .map_err(DecryptError::Crypto)?;
        if plaintext.len() < 32 {
            return Err(DecryptError::FrameTooShort);
        }
        let salt     = i64::from_le_bytes(plaintext[..8].try_into().unwrap());
        let sid      = i64::from_le_bytes(plaintext[8..16].try_into().unwrap());
        let msg_id   = i64::from_le_bytes(plaintext[16..24].try_into().unwrap());
        let seq_no   = i32::from_le_bytes(plaintext[24..28].try_into().unwrap());
        let body_len = u32::from_le_bytes(plaintext[28..32].try_into().unwrap()) as usize;
        if sid != session_id {
            return Err(DecryptError::SessionMismatch);
        }
        let body = plaintext[32..32 + body_len.min(plaintext.len() - 32)].to_vec();
        Ok(DecryptedMessage { salt, session_id: sid, msg_id, seq_no, body })
    }
}
