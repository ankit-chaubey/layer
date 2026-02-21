//! MTProto session and transport abstractions.
//!
//! This crate handles:
//! * Message framing (sequence numbers, message IDs)
//! * Plaintext transport (for initial handshake / key exchange)
//! * Encrypted transport skeleton (requires a crypto backend)
//!
//! It is intentionally transport-agnostic: bring your own TCP/WebSocket.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod authentication;
pub mod encrypted;
pub mod message;
pub mod session;
pub mod transport;

pub use message::{Message, MessageId};
pub use session::Session;
pub use encrypted::EncryptedSession;
pub use authentication::{Finished, step1, step2, step3, finish};
