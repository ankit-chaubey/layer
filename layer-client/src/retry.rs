//! Retry policies for handling `FLOOD_WAIT` and transient I/O errors.

use std::num::NonZeroU32;
use std::ops::ControlFlow;
use std::time::Duration;

use crate::errors::InvocationError;

/// Controls how the client reacts when an RPC call fails.
pub trait RetryPolicy: Send + Sync + 'static {
    fn should_retry(&self, ctx: &RetryContext) -> ControlFlow<(), Duration>;
}

/// Context passed to [`RetryPolicy::should_retry`] on each failure.
pub struct RetryContext {
    pub fail_count:   NonZeroU32,
    pub slept_so_far: Duration,
    pub error:        InvocationError,
}

/// Never retry.
pub struct NoRetries;
impl RetryPolicy for NoRetries {
    fn should_retry(&self, _: &RetryContext) -> ControlFlow<(), Duration> {
        ControlFlow::Break(())
    }
}

/// Automatically sleep on FLOOD_WAIT and retry once on I/O errors.
pub struct AutoSleep {
    pub threshold:             Duration,
    pub io_errors_as_flood_of: Option<Duration>,
}

impl Default for AutoSleep {
    fn default() -> Self {
        Self {
            threshold:             Duration::from_secs(60),
            io_errors_as_flood_of: Some(Duration::from_secs(1)),
        }
    }
}

impl RetryPolicy for AutoSleep {
    fn should_retry(&self, ctx: &RetryContext) -> ControlFlow<(), Duration> {
        if let Some(secs) = ctx.error.flood_wait_seconds() {
            if ctx.fail_count.get() == 1 && secs <= self.threshold.as_secs() {
                log::info!("FLOOD_WAIT_{secs} — sleeping before retry");
                return ControlFlow::Continue(Duration::from_secs(secs));
            }
        }
        if matches!(ctx.error, InvocationError::Io(_)) && ctx.fail_count.get() == 1 {
            if let Some(d) = self.io_errors_as_flood_of {
                log::info!("I/O error — sleeping {:?} before retry", d);
                return ControlFlow::Continue(d);
            }
        }
        ControlFlow::Break(())
    }
}
