//! Error types for layer-client.
//!
//! Mirrors `grammers_mtsender` error hierarchy for API compatibility.

use std::{fmt, io};

// ─── RpcError ─────────────────────────────────────────────────────────────────

/// An error returned by Telegram's servers in response to an RPC call.
///
/// Numeric values are stripped from the name and placed in [`RpcError::value`].
///
/// # Example
/// `FLOOD_WAIT_30` → `RpcError { code: 420, name: "FLOOD_WAIT", value: Some(30) }`
#[derive(Clone, Debug, PartialEq)]
pub struct RpcError {
    /// HTTP-like status code.
    pub code: i32,
    /// Error name in SCREAMING_SNAKE_CASE with digits removed.
    pub name: String,
    /// Numeric suffix extracted from the name, if any.
    pub value: Option<u32>,
}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RPC {}: {}", self.code, self.name)?;
        if let Some(v) = self.value {
            write!(f, " (value: {v})")?;
        }
        Ok(())
    }
}

impl std::error::Error for RpcError {}

impl RpcError {
    /// Parse a raw Telegram error message like `"FLOOD_WAIT_30"` into an `RpcError`.
    pub fn from_telegram(code: i32, message: &str) -> Self {
        // Try to find a numeric suffix after the last underscore.
        // e.g. "FLOOD_WAIT_30" → name = "FLOOD_WAIT", value = Some(30)
        if let Some(idx) = message.rfind('_') {
            let suffix = &message[idx + 1..];
            if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
                if let Ok(v) = suffix.parse::<u32>() {
                    let name = message[..idx].to_string();
                    return Self { code, name, value: Some(v) };
                }
            }
        }
        Self { code, name: message.to_string(), value: None }
    }

    /// Match on the error name, with optional wildcard prefix/suffix `'*'`.
    ///
    /// # Examples
    /// - `err.is("FLOOD_WAIT")` — exact match
    /// - `err.is("PHONE_CODE_*")` — starts-with match  
    /// - `err.is("*_INVALID")` — ends-with match
    pub fn is(&self, pattern: &str) -> bool {
        if let Some(prefix) = pattern.strip_suffix('*') {
            self.name.starts_with(prefix)
        } else if let Some(suffix) = pattern.strip_prefix('*') {
            self.name.ends_with(suffix)
        } else {
            self.name == pattern
        }
    }

    /// Returns the flood-wait duration in seconds, if this is a FLOOD_WAIT error.
    pub fn flood_wait_seconds(&self) -> Option<u64> {
        if self.code == 420 && self.name == "FLOOD_WAIT" {
            self.value.map(|v| v as u64)
        } else {
            None
        }
    }
}

// ─── InvocationError ──────────────────────────────────────────────────────────

/// The error type returned from any `Client` method that talks to Telegram.
#[derive(Debug)]
pub enum InvocationError {
    /// Telegram rejected the request.
    Rpc(RpcError),
    /// Network / I/O failure.
    Io(io::Error),
    /// Response deserialization failed.
    Deserialize(String),
    /// The request was dropped (e.g. sender task shut down).
    Dropped,
    /// DC migration required — internal, automatically handled by [`crate::Client`].
    Migrate(i32),
}

impl fmt::Display for InvocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rpc(e)          => write!(f, "{e}"),
            Self::Io(e)           => write!(f, "I/O error: {e}"),
            Self::Deserialize(s)  => write!(f, "deserialize error: {s}"),
            Self::Dropped         => write!(f, "request dropped"),
            Self::Migrate(dc)     => write!(f, "DC migration to {dc}"),
        }
    }
}

impl std::error::Error for InvocationError {}

impl From<io::Error> for InvocationError {
    fn from(e: io::Error) -> Self { Self::Io(e) }
}

impl From<layer_tl_types::deserialize::Error> for InvocationError {
    fn from(e: layer_tl_types::deserialize::Error) -> Self { Self::Deserialize(e.to_string()) }
}

impl InvocationError {
    /// Returns `true` if this is the named RPC error (supports `'*'` wildcards).
    pub fn is(&self, pattern: &str) -> bool {
        match self {
            Self::Rpc(e) => e.is(pattern),
            _            => false,
        }
    }

    /// If this is a FLOOD_WAIT error, returns how many seconds to wait.
    pub fn flood_wait_seconds(&self) -> Option<u64> {
        match self {
            Self::Rpc(e) => e.flood_wait_seconds(),
            _            => None,
        }
    }
}

// ─── SignInError ──────────────────────────────────────────────────────────────

/// Errors returned by [`crate::Client::sign_in`].
#[derive(Debug)]
pub enum SignInError {
    /// The phone number is new — must sign up via the official Telegram app first.
    SignUpRequired,
    /// 2FA is enabled; the contained token must be passed to [`crate::Client::check_password`].
    PasswordRequired(PasswordToken),
    /// The code entered was wrong or has expired.
    InvalidCode,
    /// Any other error.
    Other(InvocationError),
}

impl fmt::Display for SignInError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SignUpRequired        => write!(f, "sign up required — use official Telegram app"),
            Self::PasswordRequired(_)  => write!(f, "2FA password required"),
            Self::InvalidCode          => write!(f, "invalid or expired code"),
            Self::Other(e)             => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SignInError {}

impl From<InvocationError> for SignInError {
    fn from(e: InvocationError) -> Self { Self::Other(e) }
}

// ─── PasswordToken ────────────────────────────────────────────────────────────

/// Opaque 2FA challenge token returned in [`SignInError::PasswordRequired`].
///
/// Pass to [`crate::Client::check_password`] together with the user's password.
pub struct PasswordToken {
    pub(crate) password: layer_tl_types::types::account::Password,
}

impl PasswordToken {
    /// The password hint set by the account owner, if any.
    pub fn hint(&self) -> Option<&str> {
        self.password.hint.as_deref()
    }
}

impl fmt::Debug for PasswordToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PasswordToken {{ hint: {:?} }}", self.hint())
    }
}

// ─── LoginToken ───────────────────────────────────────────────────────────────

/// Opaque token returned by [`crate::Client::request_login_code`].
///
/// Pass to [`crate::Client::sign_in`] with the received code.
pub struct LoginToken {
    pub(crate) phone:           String,
    pub(crate) phone_code_hash: String,
}
