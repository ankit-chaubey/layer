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
#![doc(html_root_url = "https://docs.rs/layer-tl-gen/0.5.0")]
//! Build-time code generator that transforms a parsed TL schema into Rust source files.
//!
//! Intended to be used from a `build.rs` script.
//!
//! # Usage
//!
//! ```no_run
//! // build.rs
//! use layer_tl_gen::{Config, Outputs, generate};
//! use layer_tl_parser::parse_tl_file;
//! use std::fs;
//!
//! fn main() {
//! let schema = fs::read_to_string("tl/api.tl").unwrap();
//! let defs: Vec<_> = parse_tl_file(&schema)
//!     .filter_map(|r| r.ok())
//!     .collect();
//!
//! let out = std::env::var("OUT_DIR").unwrap();
//! let mut outputs = Outputs::from_dir(&out).unwrap();
//! generate(&defs, &Config::default(), &mut outputs).unwrap();
//! }
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod codegen;
mod grouper;
mod metadata;
mod namegen;

pub use codegen::{Config, Outputs, generate};
