// Copyright (c) Ankit Chaubey <ankitchaubey.dev@gmail.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

// NOTE:
// The "Layer" project is no longer maintained or supported.
// Its original purpose for personal SDK/APK experimentation and learning
// has been fulfilled.
//
// Please use Ferogram instead:
// https://github.com/ankit-chaubey/ferogram
// Ferogram will receive future updates and development, although progress
// may be slower.
//
// Ferogram is an async Telegram MTProto client library written in Rust.
// Its implementation follows the behaviour of the official Telegram clients,
// particularly Telegram Desktop and TDLib, and aims to provide a clean and
// modern async interface for building Telegram clients and tools.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(html_root_url = "https://docs.rs/layer-mtproto/0.5.0")]
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

pub use authentication::{Finished, finish, step1, step2, step3};
pub use encrypted::EncryptedSession;
pub use message::{Message, MessageId};
pub use session::Session;
