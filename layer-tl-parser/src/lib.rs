//! Parser for Telegram's [Type Language] (TL) schema files.
//!
//! This crate converts raw `.tl` text into a structured [`Definition`] AST
//! which can then be used by code-generators (see `layer-tl-gen`).
//!
//! # Quick start
//!
//! ```rust
//! use layer_tl_parser::parse_tl_file;
//!
//! let src = "user#12345 id:long name:string = User;";
//! for def in parse_tl_file(src) {
//!     println!("{:#?}", def.unwrap());
//! }
//! ```
//!
//! [Type Language]: https://core.telegram.org/mtproto/TL

#![deny(unsafe_code)]
#![warn(missing_docs)]

/// Parse error types for TL schema parsing.
pub mod errors;
pub mod tl;
mod iterator;
mod utils;

use errors::ParseError;
use tl::Definition;

/// Parses a complete TL schema file, yielding [`Definition`]s one by one.
///
/// Lines starting with `//` are treated as comments and skipped.
/// The special `---functions---` and `---types---` section markers switch
/// the [`tl::Category`] applied to the following definitions.
///
/// Returns an iterator of `Result<Definition, ParseError>` so callers can
/// decide whether to skip or hard-fail on bad lines.
pub fn parse_tl_file(contents: &str) -> impl Iterator<Item = Result<Definition, ParseError>> + '_ {
    iterator::TlIterator::new(contents)
}
