//! [`ClientBuilder`] for constructing a [`Config`] and connecting.
//!
//! # Example
//! ```rust,no_run
//! use layer_client::Client;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//! let (client, _shutdown) = Client::builder()
//!     .api_id(12345)
//!     .api_hash("abc123")
//!     .session("my.session")
//!     .catch_up(true)
//!     .connect().await?;
//! Ok(())
//! }
//! ```
//!
//! Use `.session_string(s)` instead of `.session(path)` for portable base64 sessions:
//! ```rust,no_run
//! # use layer_client::Client;
//! # #[tokio::main] async fn main() -> anyhow::Result<()> {
//! let (client, _shutdown) = Client::builder()
//! .api_id(12345)
//! .api_hash("abc123")
//! .session_string(std::env::var("SESSION").unwrap_or_default())
//! .connect().await?;
//! # Ok(()) }
//! ```

use std::sync::Arc;

use crate::{
    Client, Config, InvocationError, ShutdownToken, TransportKind,
    retry::{AutoSleep, RetryPolicy},
    session_backend::{BinaryFileBackend, InMemoryBackend, SessionBackend, StringSessionBackend},
    socks5::Socks5Config,
};

/// Fluent builder for [`Config`] + [`Client::connect`].
///
/// Obtain one via [`Client::builder()`].
pub struct ClientBuilder {
    api_id: i32,
    api_hash: String,
    dc_addr: Option<String>,
    retry_policy: Arc<dyn RetryPolicy>,
    socks5: Option<Socks5Config>,
    allow_ipv6: bool,
    transport: TransportKind,
    session_backend: Arc<dyn SessionBackend>,
    catch_up: bool,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            api_id: 0,
            api_hash: String::new(),
            dc_addr: None,
            retry_policy: Arc::new(AutoSleep::default()),
            socks5: None,
            allow_ipv6: false,
            transport: TransportKind::Abridged,
            session_backend: Arc::new(BinaryFileBackend::new("layer.session")),
            catch_up: false,
        }
    }
}

impl ClientBuilder {
    // Credentials

    /// Set the Telegram API ID (from <https://my.telegram.org>).
    pub fn api_id(mut self, id: i32) -> Self {
        self.api_id = id;
        self
    }

    /// Set the Telegram API hash (from <https://my.telegram.org>).
    pub fn api_hash(mut self, hash: impl Into<String>) -> Self {
        self.api_hash = hash.into();
        self
    }

    // Session

    /// Use a binary file session at `path`.
    ///
    /// Mutually exclusive with [`session_string`](Self::session_string) and
    /// [`in_memory`](Self::in_memory): last call wins.
    pub fn session(mut self, path: impl AsRef<std::path::Path>) -> Self {
        self.session_backend = Arc::new(BinaryFileBackend::new(path.as_ref()));
        self
    }

    /// Use a portable base64 string session.
    ///
    /// Pass an empty string to start fresh: the exported session string
    /// from [`Client::export_session_string`] can be injected here directly
    /// (e.g. via an environment variable).
    ///
    /// Mutually exclusive with [`session`](Self::session) and
    /// [`in_memory`](Self::in_memory): last call wins.
    pub fn session_string(mut self, s: impl Into<String>) -> Self {
        self.session_backend = Arc::new(StringSessionBackend::new(s));
        self
    }

    /// Use a non-persistent in-memory session (useful for tests).
    ///
    /// Mutually exclusive with [`session`](Self::session) and
    /// [`session_string`](Self::session_string): last call wins.
    pub fn in_memory(mut self) -> Self {
        self.session_backend = Arc::new(InMemoryBackend::new());
        self
    }

    /// Inject a fully custom [`SessionBackend`] implementation.
    ///
    /// Useful for [`LibSqlBackend`] (bundled SQLite, no system dep) or any
    /// custom persistence layer:
    /// ```rust,no_run
    /// # use layer_client::{Client};
    /// # #[cfg(feature = "libsql-session")] {
    /// # use layer_client::LibSqlBackend;
    /// use std::sync::Arc;
    /// let (client, _) = Client::builder()
    /// .api_id(12345).api_hash("abc")
    /// .session_backend(Arc::new(LibSqlBackend::new("my.db")))
    /// .connect().await?;
    /// # }
    /// ```
    pub fn session_backend(mut self, backend: Arc<dyn SessionBackend>) -> Self {
        self.session_backend = backend;
        self
    }

    // Update catch-up

    /// When `true`, replay missed updates via `updates.getDifference` on connect.
    ///
    /// Default: `false`.
    pub fn catch_up(mut self, enabled: bool) -> Self {
        self.catch_up = enabled;
        self
    }

    // Network

    /// Override the first DC address (e.g. `"149.154.167.51:443"`).
    pub fn dc_addr(mut self, addr: impl Into<String>) -> Self {
        self.dc_addr = Some(addr.into());
        self
    }

    /// Route all connections through a SOCKS5 proxy.
    pub fn socks5(mut self, proxy: Socks5Config) -> Self {
        self.socks5 = Some(proxy);
        self
    }

    /// Allow IPv6 DC addresses (default: `false`).
    pub fn allow_ipv6(mut self, allow: bool) -> Self {
        self.allow_ipv6 = allow;
        self
    }

    /// Choose the MTProto transport framing (default: [`TransportKind::Abridged`]).
    pub fn transport(mut self, kind: TransportKind) -> Self {
        self.transport = kind;
        self
    }

    // Retry

    /// Override the flood-wait / retry policy.
    pub fn retry_policy(mut self, policy: Arc<dyn RetryPolicy>) -> Self {
        self.retry_policy = policy;
        self
    }

    // Terminal

    /// Build the [`Config`] without connecting.
    ///
    /// Useful when you need to pass `Config` to a function that calls
    /// `Client::connect` itself.
    pub fn build(self) -> Result<Config, BuilderError> {
        if self.api_id == 0 {
            return Err(BuilderError::MissingApiId);
        }
        if self.api_hash.is_empty() {
            return Err(BuilderError::MissingApiHash);
        }
        Ok(Config {
            api_id: self.api_id,
            api_hash: self.api_hash,
            dc_addr: self.dc_addr,
            retry_policy: self.retry_policy,
            socks5: self.socks5,
            allow_ipv6: self.allow_ipv6,
            transport: self.transport,
            session_backend: self.session_backend,
            catch_up: self.catch_up,
        })
    }

    /// Build and connect in one step.
    ///
    /// Returns `Err(BuilderError::MissingApiId)` / `Err(BuilderError::MissingApiHash)`
    /// before attempting any network I/O if the required fields are absent.
    pub async fn connect(self) -> Result<(Client, ShutdownToken), BuilderError> {
        let cfg = self.build()?;
        Client::connect(cfg).await.map_err(BuilderError::Connect)
    }
}

// BuilderError

/// Errors that can be returned by [`ClientBuilder::build`] or
/// [`ClientBuilder::connect`].
#[derive(Debug)]
pub enum BuilderError {
    /// `api_id` was not set (or left at 0).
    MissingApiId,
    /// `api_hash` was not set (or left empty).
    MissingApiHash,
    /// The underlying [`Client::connect`] call failed.
    Connect(InvocationError),
}

impl std::fmt::Display for BuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingApiId => f.write_str("ClientBuilder: api_id not set"),
            Self::MissingApiHash => f.write_str("ClientBuilder: api_hash not set"),
            Self::Connect(e) => write!(f, "ClientBuilder: connect failed: {e}"),
        }
    }
}

impl std::error::Error for BuilderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Connect(e) => Some(e),
            _ => None,
        }
    }
}
