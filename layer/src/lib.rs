//! # layer — Telegram MTProto library
//!
//! `layer` is a modular Rust library for the Telegram MTProto protocol.
//! It consists of four focused sub-crates wired together here for convenience:
//!
//! | Sub-crate         | Role                                              |
//! |-------------------|---------------------------------------------------|
//! | `layer-tl-parser` | Parse `.tl` schema files into an AST              |
//! | `layer-tl-gen`    | Generate Rust source from the AST (build-time)    |
//! | `layer-tl-types`  | Auto-generated types, functions & enums           |
//! | `layer-mtproto`   | Session state, message framing, transport traits  |
//!
//! ## Quick start: raw API
//!
//! ```rust,no_run
//! use layer::tl::{functions, Serializable, RemoteCall};
//! use layer::mtproto::{Session, transport::AbridgedTransport};
//!
//! // Build a raw TL request
//! let req = functions::help::GetConfig {};
//! let bytes = req.to_bytes();
//!
//! // Pack it into an MTProto message
//! let mut session = Session::new();
//! let msg = session.pack(&req);
//! let wire = msg.to_plaintext_bytes();
//!
//! // Send over your transport…
//! ```
//!
//! ## Updating the API layer
//!
//! Replace `layer-tl-types/tl/api.tl` with the new schema and run `cargo build`.
//! Everything else regenerates automatically.

#![deny(unsafe_code)]
#![warn(missing_docs)]

/// Re-export of [`layer_tl_types`] — generated constructors, functions and enums.
pub use layer_tl_types as tl;

/// Re-export of [`layer_mtproto`] — session, encrypted session, transport, and authentication.
pub use layer_mtproto as mtproto;

/// Re-export of [`layer_crypto`] — AES-IGE, SHA, RSA, factorize, AuthKey.
pub use layer_crypto as crypto;

/// Re-export of [`layer_tl_parser`] (requires `feature = "parser"`).
#[cfg(feature = "parser")]
pub use layer_tl_parser as parser;

/// Re-export of [`layer_tl_gen`] (requires `feature = "codegen"`).
#[cfg(feature = "codegen")]
pub use layer_tl_gen as codegen;

// ─── Convenience re-exports ───────────────────────────────────────────────────

pub use layer_tl_types::{
    Identifiable,
    RemoteCall,
    Serializable,
    Deserializable,
    LAYER,
};

pub use layer_mtproto::{Session, EncryptedSession};
pub use layer_mtproto::authentication::{self, Finished, step1, step2, step3, finish};
pub use layer_crypto::AuthKey;
