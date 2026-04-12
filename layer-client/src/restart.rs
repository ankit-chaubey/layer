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

use std::time::Duration;

pub trait ConnectionRestartPolicy: Send + Sync + 'static {
    fn restart_interval(&self) -> Option<Duration>;
}

pub struct NeverRestart;

impl ConnectionRestartPolicy for NeverRestart {
    fn restart_interval(&self) -> Option<Duration> {
        None
    }
}

pub struct FixedInterval {
    pub interval: Duration,
}

impl ConnectionRestartPolicy for FixedInterval {
    fn restart_interval(&self) -> Option<Duration> {
        Some(self.interval)
    }
}
