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

//! Rust types representing the Abstract Syntax Tree of a TL definition.

mod category;
mod definition;
mod flag;
mod parameter;
mod parameter_type;
mod ty;

pub use category::Category;
pub use definition::Definition;
pub use flag::Flag;
pub use parameter::Parameter;
pub use parameter_type::ParameterType;
pub use ty::Type;
