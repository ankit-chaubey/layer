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

//! Groups definitions by namespace and return type for organised code output.

use layer_tl_parser::tl::{Category, Definition, Type};
use std::collections::HashMap;

/// Group definitions of `category` by their (first-level) namespace.
pub(crate) fn group_by_ns(
    defs: &[Definition],
    category: Category,
) -> HashMap<String, Vec<&Definition>> {
    let mut map: HashMap<String, Vec<&Definition>> = HashMap::new();

    for def in defs.iter().filter(|d| d.category == category) {
        assert!(
            def.namespace.len() <= 1,
            "only one namespace level supported"
        );
        let ns = def.namespace.first().map(|s| s.as_str()).unwrap_or("");
        map.entry(ns.to_owned()).or_default().push(def);
    }

    // Sort each bucket alphabetically for deterministic output
    for bucket in map.values_mut() {
        bucket.sort_by_key(|d| &d.name);
    }

    map
}

/// Group the *return types* of constructors by namespace.
/// Used to emit `enum` blocks.
pub(crate) fn group_types_by_ns(defs: &[Definition]) -> HashMap<Option<String>, Vec<&Type>> {
    let mut map: HashMap<Option<String>, Vec<&Type>> = HashMap::new();

    for def in defs
        .iter()
        .filter(|d| d.category == Category::Types && !d.ty.generic_ref)
    {
        assert!(def.namespace.len() <= 1);
        map.entry(def.namespace.first().cloned())
            .or_default()
            .push(&def.ty);
    }

    for bucket in map.values_mut() {
        bucket.sort_by_key(|t| &t.name);
        bucket.dedup_by_key(|t| &t.name);
    }

    map
}
