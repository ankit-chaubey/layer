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
//!     let schema = fs::read_to_string("tl/api.tl").unwrap();
//!     let defs: Vec<_> = parse_tl_file(&schema)
//!         .filter_map(|r| r.ok())
//!         .collect();
//!
//!     let out = std::env::var("OUT_DIR").unwrap();
//!     let mut outputs = Outputs::from_dir(&out).unwrap();
//!     generate(&defs, &Config::default(), &mut outputs).unwrap();
//! }
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]

mod grouper;
mod metadata;
mod namegen;
pub mod codegen;

pub use codegen::{generate, Config, Outputs};
