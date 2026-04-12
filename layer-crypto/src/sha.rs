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

/// Calculate the SHA-1 hash of one or more byte slices concatenated.
#[macro_export]
macro_rules! sha1 {
    ( $( $x:expr ),+ ) => {{
        use sha1::{Digest, Sha1};
        let mut h = Sha1::new();
        $( h.update($x); )+
        let out: [u8; 20] = h.finalize().into();
        out
    }};
}

/// Calculate the SHA-256 hash of one or more byte slices concatenated.
#[macro_export]
macro_rules! sha256 {
    ( $( $x:expr ),+ ) => {{
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        $( h.update($x); )+
        let out: [u8; 32] = h.finalize().into();
        out
    }};
}
