//! # layer-client
//!
//! Production-grade async Telegram client built on MTProto.
//!
//! ## Features
//! - User login (phone + code + 2FA SRP) and bot token login
//! - Peer access-hash caching — API calls always carry correct access hashes
//! - `FLOOD_WAIT` auto-retry with configurable policy
//! - Typed async update stream: `NewMessage`, `MessageEdited`, `MessageDeleted`,
//!   `CallbackQuery`, `InlineQuery`, `InlineSend`, `Raw`
//! - Send / edit / delete / forward / pin messages
//! - Search messages (per-chat and global)
//! - Mark as read, delete dialogs, clear mentions
//! - Join chat / accept invite links
//! - Chat action (typing, uploading, …)
//! - `get_me()` — fetch own User info
//! - Paginated dialog and message iterators
//! - DC migration, session persistence, reconnect

#![deny(unsafe_code)]

mod errors;
mod retry;
mod session;
mod transport;
mod two_factor_auth;
pub mod update;
pub mod parsers;
pub mod media;
pub mod participants;
pub mod pts;

// ── New feature modules ───────────────────────────────────────────────────────
pub mod dc_pool;
pub mod transport_obfuscated;
pub mod transport_intermediate;
pub mod socks5;
pub mod session_backend;
pub mod inline_iter;
pub mod typing_guard;

pub use errors::{InvocationError, LoginToken, PasswordToken, RpcError, SignInError};
pub use retry::{AutoSleep, NoRetries, RetryContext, RetryPolicy};
pub use update::Update;
pub use media::{UploadedFile, DownloadIter};
pub use participants::Participant;
pub use typing_guard::TypingGuard;
pub use socks5::Socks5Config;
pub use session_backend::{SessionBackend, BinaryFileBackend, InMemoryBackend};

use std::collections::HashMap;
use std::collections::VecDeque;
use std::num::NonZeroU32;
use std::ops::ControlFlow;
use std::sync::Arc;
use std::time::Duration;

use layer_tl_types as tl;
use layer_mtproto::{EncryptedSession, Session, authentication as auth};
use layer_tl_types::{Cursor, Deserializable, RemoteCall};
use session::{DcEntry, PersistedSession};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};
use tokio::time::sleep;

// ─── MTProto envelope constructor IDs ────────────────────────────────────────

const ID_RPC_RESULT:       u32 = 0xf35c6d01;
const ID_RPC_ERROR:        u32 = 0x2144ca19;
const ID_MSG_CONTAINER:    u32 = 0x73f1f8dc;
const ID_GZIP_PACKED:      u32 = 0x3072cfa1;
const ID_PONG:             u32 = 0x347773c5;
const ID_MSGS_ACK:         u32 = 0x62d6b459;
const ID_BAD_SERVER_SALT:  u32 = 0xedab447b;
const ID_NEW_SESSION:      u32 = 0x9ec20908;
const ID_BAD_MSG_NOTIFY:   u32 = 0xa7eff811;
const ID_UPDATES:          u32 = 0x74ae4240;
const ID_UPDATE_SHORT:     u32 = 0x78d4dec1;
const ID_UPDATES_COMBINED: u32 = 0x725b04c3;
const ID_UPDATE_SHORT_MSG:      u32 = 0x313bc7f8;
const ID_UPDATE_SHORT_CHAT_MSG: u32 = 0x4d6deea5;
const ID_UPDATES_TOO_LONG:      u32 = 0xe317af7e;

// ─── PeerCache ────────────────────────────────────────────────────────────────

/// Caches access hashes for users and channels so every API call carries the
/// correct hash without re-resolving peers.
#[derive(Default)]
pub(crate) struct PeerCache {
    /// user_id → access_hash
    pub(crate) users:    HashMap<i64, i64>,
    /// channel_id → access_hash
    pub(crate) channels: HashMap<i64, i64>,
}

impl PeerCache {
    fn cache_user(&mut self, user: &tl::enums::User) {
        if let tl::enums::User::User(u) = user {
            if let Some(hash) = u.access_hash {
                self.users.insert(u.id, hash);
            }
        }
    }

    fn cache_chat(&mut self, chat: &tl::enums::Chat) {
        match chat {
            tl::enums::Chat::Channel(c) => {
                if let Some(hash) = c.access_hash {
                    self.channels.insert(c.id, hash);
                }
            }
            tl::enums::Chat::ChannelForbidden(c) => {
                self.channels.insert(c.id, c.access_hash);
            }
            _ => {}
        }
    }

    fn cache_users(&mut self, users: &[tl::enums::User]) {
        for u in users { self.cache_user(u); }
    }

    fn cache_chats(&mut self, chats: &[tl::enums::Chat]) {
        for c in chats { self.cache_chat(c); }
    }

    fn user_input_peer(&self, user_id: i64) -> tl::enums::InputPeer {
        if user_id == 0 {
            return tl::enums::InputPeer::PeerSelf;
        }
        let hash = self.users.get(&user_id).copied().unwrap_or(0);
        tl::enums::InputPeer::User(tl::types::InputPeerUser { user_id, access_hash: hash })
    }

    fn channel_input_peer(&self, channel_id: i64) -> tl::enums::InputPeer {
        let hash = self.channels.get(&channel_id).copied().unwrap_or(0);
        tl::enums::InputPeer::Channel(tl::types::InputPeerChannel { channel_id, access_hash: hash })
    }

    fn peer_to_input(&self, peer: &tl::enums::Peer) -> tl::enums::InputPeer {
        match peer {
            tl::enums::Peer::User(u) => self.user_input_peer(u.user_id),
            tl::enums::Peer::Chat(c) => tl::enums::InputPeer::Chat(
                tl::types::InputPeerChat { chat_id: c.chat_id }
            ),
            tl::enums::Peer::Channel(c) => self.channel_input_peer(c.channel_id),
        }
    }
}

// ─── InputMessage builder ─────────────────────────────────────────────────────

/// Builder for composing outgoing messages.
///
/// ```rust,no_run
/// use layer_client::InputMessage;
///
/// let msg = InputMessage::text("Hello, *world*!")
///     .silent(true)
///     .reply_to(Some(42));
/// ```
#[derive(Clone, Default)]
pub struct InputMessage {
    pub text:         String,
    pub reply_to:     Option<i32>,
    pub silent:       bool,
    pub background:   bool,
    pub clear_draft:  bool,
    pub no_webpage:   bool,
    pub entities:     Option<Vec<tl::enums::MessageEntity>>,
    pub reply_markup: Option<tl::enums::ReplyMarkup>,
    pub schedule_date: Option<i32>,
}

impl InputMessage {
    /// Create a message with the given text.
    pub fn text(text: impl Into<String>) -> Self {
        Self { text: text.into(), ..Default::default() }
    }

    /// Set the message text.
    pub fn set_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into(); self
    }

    /// Reply to a specific message ID.
    pub fn reply_to(mut self, id: Option<i32>) -> Self {
        self.reply_to = id; self
    }

    /// Send silently (no notification sound).
    pub fn silent(mut self, v: bool) -> Self {
        self.silent = v; self
    }

    /// Send in background.
    pub fn background(mut self, v: bool) -> Self {
        self.background = v; self
    }

    /// Clear the draft after sending.
    pub fn clear_draft(mut self, v: bool) -> Self {
        self.clear_draft = v; self
    }

    /// Disable link preview.
    pub fn no_webpage(mut self, v: bool) -> Self {
        self.no_webpage = v; self
    }

    /// Attach formatting entities (bold, italic, code, links, etc).
    pub fn entities(mut self, e: Vec<tl::enums::MessageEntity>) -> Self {
        self.entities = Some(e); self
    }

    /// Attach a reply markup (inline or reply keyboard).
    pub fn reply_markup(mut self, rm: tl::enums::ReplyMarkup) -> Self {
        self.reply_markup = Some(rm); self
    }

    /// Schedule the message for a future Unix timestamp.
    pub fn schedule_date(mut self, ts: Option<i32>) -> Self {
        self.schedule_date = ts; self
    }

    fn reply_header(&self) -> Option<tl::enums::InputReplyTo> {
        self.reply_to.map(|id| {
            tl::enums::InputReplyTo::Message(
                tl::types::InputReplyToMessage {
                    reply_to_msg_id: id,
                    top_msg_id: None,
                    reply_to_peer_id: None,
                    quote_text: None,
                    quote_entities: None,
                    quote_offset: None,
                    monoforum_peer_id: None,
                    todo_item_id: None,
                }
            )
        })
    }
}

impl From<&str> for InputMessage {
    fn from(s: &str) -> Self { Self::text(s) }
}

impl From<String> for InputMessage {
    fn from(s: String) -> Self { Self::text(s) }
}

// ─── TransportKind ────────────────────────────────────────────────────────────

/// Which MTProto transport framing to use for all connections.
///
/// | Variant | Init bytes | Notes |
/// |---------|-----------|-------|
/// | `Abridged` | `0xef` | Default, smallest overhead |
/// | `Intermediate` | `0xeeeeeeee` | Better proxy compat |
/// | `Full` | none | Adds seqno + CRC32 |
/// | `Obfuscated` | random 64B | Bypasses DPI / MTProxy |
#[derive(Clone, Debug, Default)]
pub enum TransportKind {
    /// MTProto [Abridged] transport — length prefix is 1 or 4 bytes.
    ///
    /// [Abridged]: https://core.telegram.org/mtproto/mtproto-transports#abridged
    #[default]
    Abridged,
    /// MTProto [Intermediate] transport — 4-byte LE length prefix.
    ///
    /// [Intermediate]: https://core.telegram.org/mtproto/mtproto-transports#intermediate
    Intermediate,
    /// MTProto [Full] transport — 4-byte length + seqno + CRC32.
    ///
    /// [Full]: https://core.telegram.org/mtproto/mtproto-transports#full
    Full,
    /// [Obfuscated2] transport — XOR stream cipher over Abridged framing.
    /// Required for MTProxy and networks with deep-packet inspection.
    ///
    /// `secret` is the 16-byte proxy secret, or `None` for keyless obfuscation.
    ///
    /// [Obfuscated2]: https://core.telegram.org/mtproto/mtproto-transports#obfuscated-2
    Obfuscated { secret: Option<[u8; 16]> },
}

// ─── Config ───────────────────────────────────────────────────────────────────

/// Configuration for [`Client::connect`].
#[derive(Clone)]
pub struct Config {
    pub api_id:         i32,
    pub api_hash:       String,
    pub dc_addr:        Option<String>,
    pub retry_policy:   Arc<dyn RetryPolicy>,
    /// Optional SOCKS5 proxy — every Telegram connection is tunnelled through it.
    pub socks5:         Option<crate::socks5::Socks5Config>,
    /// Allow IPv6 DC addresses when populating the DC table (default: false).
    pub allow_ipv6:     bool,
    /// Which MTProto transport framing to use (default: Abridged).
    pub transport:      TransportKind,
    /// Session persistence backend (default: binary file `"layer.session"`).
    pub session_backend: Arc<dyn crate::session_backend::SessionBackend>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_id:          0,
            api_hash:        String::new(),
            dc_addr:         None,
            retry_policy:    Arc::new(AutoSleep::default()),
            socks5:          None,
            allow_ipv6:      false,
            transport:       TransportKind::Abridged,
            session_backend: Arc::new(crate::session_backend::BinaryFileBackend::new("layer.session")),
        }
    }
}

// ─── UpdateStream ─────────────────────────────────────────────────────────────

/// Asynchronous stream of [`Update`]s.
pub struct UpdateStream {
    rx: mpsc::UnboundedReceiver<update::Update>,
}

impl UpdateStream {
    /// Wait for the next update. Returns `None` when the client has disconnected.
    pub async fn next(&mut self) -> Option<update::Update> {
        self.rx.recv().await
    }
}

// ─── Dialog ───────────────────────────────────────────────────────────────────

/// A Telegram dialog (chat, user, channel).
#[derive(Debug, Clone)]
pub struct Dialog {
    pub raw:     tl::enums::Dialog,
    pub message: Option<tl::enums::Message>,
    pub entity:  Option<tl::enums::User>,
    pub chat:    Option<tl::enums::Chat>,
}

impl Dialog {
    /// The dialog's display title.
    pub fn title(&self) -> String {
        if let Some(tl::enums::User::User(u)) = &self.entity {
            let first = u.first_name.as_deref().unwrap_or("");
            let last  = u.last_name.as_deref().unwrap_or("");
            let name  = format!("{first} {last}").trim().to_string();
            if !name.is_empty() { return name; }
        }
        if let Some(chat) = &self.chat {
            return match chat {
                tl::enums::Chat::Chat(c)         => c.title.clone(),
                tl::enums::Chat::Forbidden(c) => c.title.clone(),
                tl::enums::Chat::Channel(c)      => c.title.clone(),
                tl::enums::Chat::ChannelForbidden(c) => c.title.clone(),
                tl::enums::Chat::Empty(_)        => "(empty)".into(),
            };
        }
        "(Unknown)".to_string()
    }

    /// Peer of this dialog.
    pub fn peer(&self) -> Option<&tl::enums::Peer> {
        match &self.raw {
            tl::enums::Dialog::Dialog(d) => Some(&d.peer),
            tl::enums::Dialog::Folder(_) => None,
        }
    }

    /// Unread message count.
    pub fn unread_count(&self) -> i32 {
        match &self.raw {
            tl::enums::Dialog::Dialog(d) => d.unread_count,
            _ => 0,
        }
    }

    /// ID of the top message.
    pub fn top_message(&self) -> i32 {
        match &self.raw {
            tl::enums::Dialog::Dialog(d) => d.top_message,
            _ => 0,
        }
    }
}

// ─── ClientInner ─────────────────────────────────────────────────────────────

struct ClientInner {
    conn:            Mutex<Connection>,
    home_dc_id:      Mutex<i32>,
    dc_options:      Mutex<HashMap<i32, DcEntry>>,
    pub(crate) peer_cache:    Mutex<PeerCache>,
    pub(crate) pts_state:     Mutex<pts::PtsState>,
    api_id:          i32,
    api_hash:        String,
    retry_policy:    Arc<dyn RetryPolicy>,
    socks5:          Option<crate::socks5::Socks5Config>,
    allow_ipv6:      bool,
    transport:       TransportKind,
    session_backend: Arc<dyn crate::session_backend::SessionBackend>,
    dc_pool:         Mutex<dc_pool::DcPool>,
    _update_tx:      mpsc::UnboundedSender<update::Update>,
}

/// The main Telegram client. Cheap to clone — internally Arc-wrapped.
#[derive(Clone)]
pub struct Client {
    pub(crate) inner: Arc<ClientInner>,
    _update_rx: Arc<Mutex<mpsc::UnboundedReceiver<update::Update>>>,
}

impl Client {
    // ── Connect ────────────────────────────────────────────────────────────

    pub async fn connect(config: Config) -> Result<Self, InvocationError> {
        let (update_tx, update_rx) = mpsc::unbounded_channel();

        // ── Load or fresh-connect ───────────────────────────────────────
        let socks5    = config.socks5.clone();
        let transport = config.transport.clone();

        let (conn, home_dc_id, dc_opts) =
            match config.session_backend.load()
                .map_err(InvocationError::Io)?
            {
                Some(s) => {
                    if let Some(dc) = s.dcs.iter().find(|d| d.dc_id == s.home_dc_id) {
                        if let Some(key) = dc.auth_key {
                            log::info!("[layer] Loading session (DC{}) …", s.home_dc_id);
                            match Connection::connect_with_key(
                                &dc.addr, key, dc.first_salt, dc.time_offset,
                                socks5.as_ref(), &transport,
                            ).await {
                                Ok(c) => {
                                    let mut opts = session::default_dc_addresses()
                                        .into_iter()
                                        .map(|(id, addr)| (id, DcEntry { dc_id: id, addr, auth_key: None, first_salt: 0, time_offset: 0 }))
                                        .collect::<HashMap<_, _>>();
                                    for d in &s.dcs { opts.insert(d.dc_id, d.clone()); }
                                    (c, s.home_dc_id, opts)
                                }
                                Err(e) => {
                                    log::warn!("[layer] Session connect failed ({e}), fresh connect …");
                                    Self::fresh_connect(socks5.as_ref(), &transport).await?
                                }
                            }
                        } else {
                            Self::fresh_connect(socks5.as_ref(), &transport).await?
                        }
                    } else {
                        Self::fresh_connect(socks5.as_ref(), &transport).await?
                    }
                }
                None => Self::fresh_connect(socks5.as_ref(), &transport).await?,
            };

        // ── Build DC pool ───────────────────────────────────────────────
        let pool = dc_pool::DcPool::new(home_dc_id, &dc_opts.values().cloned().collect::<Vec<_>>());

        let inner = Arc::new(ClientInner {
            conn:            Mutex::new(conn),
            home_dc_id:      Mutex::new(home_dc_id),
            dc_options:      Mutex::new(dc_opts),
            peer_cache:      Mutex::new(PeerCache::default()),
            pts_state:       Mutex::new(pts::PtsState::default()),
            api_id:          config.api_id,
            api_hash:        config.api_hash,
            retry_policy:    config.retry_policy,
            socks5:          config.socks5,
            allow_ipv6:      config.allow_ipv6,
            transport:       config.transport,
            session_backend: config.session_backend,
            dc_pool:         Mutex::new(pool),
            _update_tx:      update_tx,
        });

        let client = Self {
            inner,
            _update_rx: Arc::new(Mutex::new(update_rx)),
        };

        // If init_connection fails (e.g. stale auth key rejected by Telegram),
        // drop the saved connection, do a fresh DH handshake, and retry once.
        // This mirrors what grammers does: on a 404/bad-auth response it reconnects
        // with a brand-new auth key rather than giving up or hanging.
        if let Err(e) = client.init_connection().await {
            log::warn!("[layer] init_connection failed ({e}), retrying with fresh connect …");

            let socks5_r    = client.inner.socks5.clone();
            let transport_r = client.inner.transport.clone();
            let (new_conn, new_dc_id, new_opts) =
                Self::fresh_connect(socks5_r.as_ref(), &transport_r).await?;

            {
                let mut conn_guard = client.inner.conn.lock().await;
                *conn_guard = new_conn;
            }
            {
                let mut dc_guard = client.inner.home_dc_id.lock().await;
                *dc_guard = new_dc_id;
            }
            {
                let mut opts_guard = client.inner.dc_options.lock().await;
                *opts_guard = new_opts;
            }

            client.init_connection().await?;
        }

        let _ = client.sync_pts_state().await;
        Ok(client)
    }

    async fn fresh_connect(
        socks5:    Option<&crate::socks5::Socks5Config>,
        transport: &TransportKind,
    ) -> Result<(Connection, i32, HashMap<i32, DcEntry>), InvocationError> {
        log::info!("[layer] Fresh connect to DC2 …");
        let conn = Connection::connect_raw("149.154.167.51:443", socks5, transport).await?;
        let opts = session::default_dc_addresses()
            .into_iter()
            .map(|(id, addr)| (id, DcEntry { dc_id: id, addr, auth_key: None, first_salt: 0, time_offset: 0 }))
            .collect();
        Ok((conn, 2, opts))
    }

    // ── Session ────────────────────────────────────────────────────────────

    pub async fn save_session(&self) -> Result<(), InvocationError> {
        let conn_guard = self.inner.conn.lock().await;
        let home_dc_id = *self.inner.home_dc_id.lock().await;
        let dc_options = self.inner.dc_options.lock().await;

        let mut dcs: Vec<DcEntry> = dc_options.values().map(|e| DcEntry {
            dc_id:       e.dc_id,
            addr:        e.addr.clone(),
            auth_key:    if e.dc_id == home_dc_id { Some(conn_guard.auth_key_bytes()) } else { e.auth_key },
            first_salt:  if e.dc_id == home_dc_id { conn_guard.first_salt() } else { e.first_salt },
            time_offset: if e.dc_id == home_dc_id { conn_guard.time_offset() } else { e.time_offset },
        }).collect();
        // Collect auth keys from worker DCs in the pool
        self.inner.dc_pool.lock().await.collect_keys(&mut dcs);

        self.inner.session_backend
            .save(&PersistedSession { home_dc_id, dcs })
            .map_err(InvocationError::Io)?;
        log::info!("[layer] Session saved ✓");
        Ok(())
    }

    // ── Auth ───────────────────────────────────────────────────────────────

    /// Returns `true` if the client is already authorized.
    pub async fn is_authorized(&self) -> Result<bool, InvocationError> {
        match self.invoke(&tl::functions::updates::GetState {}).await {
            Ok(_)  => Ok(true),
            Err(e) if e.is("AUTH_KEY_UNREGISTERED")
                   || matches!(&e, InvocationError::Rpc(r) if r.code == 401) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Sign in as a bot.
    pub async fn bot_sign_in(&self, token: &str) -> Result<String, InvocationError> {
        let req = tl::functions::auth::ImportBotAuthorization {
            flags: 0, api_id: self.inner.api_id,
            api_hash: self.inner.api_hash.clone(),
            bot_auth_token: token.to_string(),
        };

        let result = match self.invoke(&req).await {
            Ok(x) => x,
            Err(InvocationError::Rpc(ref r)) if r.code == 303 => {
                let dc_id = r.value.unwrap_or(2) as i32;
                self.migrate_to(dc_id).await?;
                self.invoke(&req).await?
            }
            Err(e) => return Err(e),
        };

        let name = match result {
            tl::enums::auth::Authorization::Authorization(a) => {
                self.cache_user(&a.user).await;
                Self::extract_user_name(&a.user)
            }
            tl::enums::auth::Authorization::SignUpRequired(_) => {
                panic!("unexpected SignUpRequired during bot sign-in")
            }
        };
        log::info!("[layer] Bot signed in ✓  ({name})");
        Ok(name)
    }

    /// Request a login code for a user account.
    pub async fn request_login_code(&self, phone: &str) -> Result<LoginToken, InvocationError> {
        use tl::enums::auth::SentCode;

        let req = self.make_send_code_req(phone);
        let body = match self.rpc_call_raw(&req).await {
            Ok(b) => b,
            Err(InvocationError::Rpc(ref r)) if r.code == 303 => {
                let dc_id = r.value.unwrap_or(2) as i32;
                self.migrate_to(dc_id).await?;
                self.rpc_call_raw(&req).await?
            }
            Err(e) => return Err(e),
        };

        let mut cur = Cursor::from_slice(&body);
        let hash = match tl::enums::auth::SentCode::deserialize(&mut cur)? {
            SentCode::SentCode(s) => s.phone_code_hash,
            SentCode::Success(_)  => return Err(InvocationError::Deserialize("unexpected Success".into())),
            SentCode::PaymentRequired(_) => return Err(InvocationError::Deserialize("payment required to send code".into())),
        };
        log::info!("[layer] Login code sent");
        Ok(LoginToken { phone: phone.to_string(), phone_code_hash: hash })
    }

    /// Complete sign-in with the code sent to the phone.
    pub async fn sign_in(&self, token: &LoginToken, code: &str) -> Result<String, SignInError> {
        let req = tl::functions::auth::SignIn {
            phone_number:    token.phone.clone(),
            phone_code_hash: token.phone_code_hash.clone(),
            phone_code:      Some(code.trim().to_string()),
            email_verification: None,
        };

        let body = match self.rpc_call_raw(&req).await {
            Ok(b) => b,
            Err(InvocationError::Rpc(ref r)) if r.code == 303 => {
                let dc_id = r.value.unwrap_or(2) as i32;
                self.migrate_to(dc_id).await.map_err(SignInError::Other)?;
                self.rpc_call_raw(&req).await.map_err(SignInError::Other)?
            }
            Err(e) if e.is("SESSION_PASSWORD_NEEDED") => {
                let t = self.get_password_info().await.map_err(SignInError::Other)?;
                return Err(SignInError::PasswordRequired(t));
            }
            Err(e) if e.is("PHONE_CODE_*") => return Err(SignInError::InvalidCode),
            Err(e) => return Err(SignInError::Other(e)),
        };

        let mut cur = Cursor::from_slice(&body);
        match tl::enums::auth::Authorization::deserialize(&mut cur)
            .map_err(|e| SignInError::Other(e.into()))?
        {
            tl::enums::auth::Authorization::Authorization(a) => {
                self.cache_user(&a.user).await;
                let name = Self::extract_user_name(&a.user);
                log::info!("[layer] Signed in ✓  Welcome, {name}!");
                Ok(name)
            }
            tl::enums::auth::Authorization::SignUpRequired(_) => Err(SignInError::SignUpRequired),
        }
    }

    /// Complete 2FA login.
    pub async fn check_password(
        &self,
        token:    PasswordToken,
        password: impl AsRef<[u8]>,
    ) -> Result<String, InvocationError> {
        let pw   = token.password;
        let algo = pw.current_algo.ok_or_else(|| InvocationError::Deserialize("no current_algo".into()))?;
        let (salt1, salt2, p, g) = Self::extract_password_params(&algo)?;
        let g_b  = pw.srp_b.ok_or_else(|| InvocationError::Deserialize("no srp_b".into()))?;
        let a    = pw.secure_random;
        let srp_id = pw.srp_id.ok_or_else(|| InvocationError::Deserialize("no srp_id".into()))?;

        let (m1, g_a) = two_factor_auth::calculate_2fa(salt1, salt2, p, g, &g_b, &a, password.as_ref());
        let req = tl::functions::auth::CheckPassword {
            password: tl::enums::InputCheckPasswordSrp::InputCheckPasswordSrp(
                tl::types::InputCheckPasswordSrp {
                    srp_id, a: g_a.to_vec(), m1: m1.to_vec(),
                },
            ),
        };

        let body = self.rpc_call_raw(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        match tl::enums::auth::Authorization::deserialize(&mut cur)? {
            tl::enums::auth::Authorization::Authorization(a) => {
                self.cache_user(&a.user).await;
                let name = Self::extract_user_name(&a.user);
                log::info!("[layer] 2FA ✓  Welcome, {name}!");
                Ok(name)
            }
            tl::enums::auth::Authorization::SignUpRequired(_) =>
                Err(InvocationError::Deserialize("unexpected SignUpRequired after 2FA".into())),
        }
    }

    /// Sign out and invalidate the current session.
    pub async fn sign_out(&self) -> Result<bool, InvocationError> {
        let req = tl::functions::auth::LogOut {};
        match self.rpc_call_raw(&req).await {
            Ok(_) => { log::info!("[layer] Signed out ✓"); Ok(true) }
            Err(e) if e.is("AUTH_KEY_UNREGISTERED") => Ok(false),
            Err(e) => Err(e),
        }
    }

    // ── Get self ───────────────────────────────────────────────────────────

    /// Fetch information about the logged-in user.
    pub async fn get_me(&self) -> Result<tl::types::User, InvocationError> {
        let req = tl::functions::users::GetUsers {
            id: vec![tl::enums::InputUser::UserSelf],
        };
        let body    = self.rpc_call_raw(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        let users   = Vec::<tl::enums::User>::deserialize(&mut cur)?;
        self.cache_users_slice(&users).await;
        users.into_iter().find_map(|u| match u {
            tl::enums::User::User(u) => Some(u),
            _ => None,
        }).ok_or_else(|| InvocationError::Deserialize("getUsers returned no user".into()))
    }

    // ── Updates ────────────────────────────────────────────────────────────

    /// Return an [`UpdateStream`] that yields incoming [`Update`]s.
    pub fn stream_updates(&self) -> UpdateStream {
        let (tx, rx) = mpsc::unbounded_channel();
        let client = self.clone();
        tokio::spawn(async move {
            client.run_update_loop(tx).await;
        });
        UpdateStream { rx }
    }

    async fn run_update_loop(&self, tx: mpsc::UnboundedSender<update::Update>) {
        loop {
            let result = {
                let mut conn = self.inner.conn.lock().await;
                match tokio::time::timeout(Duration::from_secs(30), conn.recv_once()).await {
                    Ok(Ok(updates)) => Ok(updates),
                    Ok(Err(e))      => Err(e),
                    Err(_timeout)   => {
                        let _ = conn.send_ping().await;
                        continue;
                    }
                }
            };

            match result {
                Ok(updates) => {
                    for u in updates { let _ = tx.send(u); }
                }
                Err(e) => {
                    log::warn!("[layer] Update loop error: {e} — reconnecting …");
                    sleep(Duration::from_secs(1)).await;
                    let home_dc_id = *self.inner.home_dc_id.lock().await;
                    let (addr, saved_key, first_salt, time_offset) = {
                        let opts = self.inner.dc_options.lock().await;
                        match opts.get(&home_dc_id) {
                            Some(e) => (e.addr.clone(), e.auth_key, e.first_salt, e.time_offset),
                            None    => ("149.154.167.51:443".to_string(), None, 0, 0),
                        }
                    };
                    let socks5    = self.inner.socks5.clone();
                    let transport = self.inner.transport.clone();

                    // Prefer reconnecting with the existing auth key (user is already
                    // authorised on it).  Only fall back to a fresh DH if that fails.
                    let new_conn_result = if let Some(key) = saved_key {
                        log::info!("[layer] Reconnecting to DC{home_dc_id} with saved key …");
                        match Connection::connect_with_key(
                            &addr, key, first_salt, time_offset,
                            socks5.as_ref(), &transport,
                        ).await {
                            Ok(c)  => Ok(c),
                            Err(e2) => {
                                log::warn!("[layer] connect_with_key failed ({e2}), falling back to fresh DH …");
                                Connection::connect_raw(&addr, socks5.as_ref(), &transport).await
                            }
                        }
                    } else {
                        Connection::connect_raw(&addr, socks5.as_ref(), &transport).await
                    };

                    match new_conn_result {
                        Ok(new_conn) => {
                            *self.inner.conn.lock().await = new_conn;
                            if let Err(e2) = self.init_connection().await {
                                log::warn!("[layer] init_connection after reconnect failed: {e2}");
                            }
                            // Fetch any updates missed during disconnect
                            match self.get_difference().await {
                                Ok(missed) => {
                                    for u in missed { let _ = tx.send(u); }
                                }
                                Err(e2) => log::warn!("[layer] getDifference after reconnect failed: {e2}"),
                            }
                        }
                        Err(e2) => {
                            log::error!("[layer] Reconnect failed: {e2}");
                            break;
                        }
                    }
                }
            }
        }
    }

    // ── Messaging ──────────────────────────────────────────────────────────

    /// Send a text message. Use `"me"` for Saved Messages.
    pub async fn send_message(&self, peer: &str, text: &str) -> Result<(), InvocationError> {
        let p = self.resolve_peer(peer).await?;
        self.send_message_to_peer(p, text).await
    }

    /// Send a message to an already-resolved peer (plain text shorthand).
    pub async fn send_message_to_peer(
        &self,
        peer: tl::enums::Peer,
        text: &str,
    ) -> Result<(), InvocationError> {
        self.send_message_to_peer_ex(peer, &InputMessage::text(text)).await
    }

    /// Send a message with full [`InputMessage`] options.
    pub async fn send_message_to_peer_ex(
        &self,
        peer: tl::enums::Peer,
        msg:  &InputMessage,
    ) -> Result<(), InvocationError> {
        let input_peer = self.inner.peer_cache.lock().await.peer_to_input(&peer);
        let req = tl::functions::messages::SendMessage {
            no_webpage:               msg.no_webpage,
            silent:                   msg.silent,
            background:               msg.background,
            clear_draft:              msg.clear_draft,
            noforwards:               false,
            update_stickersets_order: false,
            invert_media:             false,
            allow_paid_floodskip:     false,
            peer:                     input_peer,
            reply_to:                 msg.reply_header(),
            message:                  msg.text.clone(),
            random_id:                random_i64(),
            reply_markup:             msg.reply_markup.clone(),
            entities:                 msg.entities.clone(),
            schedule_date:            msg.schedule_date,
            schedule_repeat_period:   None,
            send_as:                  None,
            quick_reply_shortcut:     None,
            effect:                   None,
            allow_paid_stars:         None,
            suggested_post:           None,
        };
        self.rpc_write(&req).await
    }

    /// Send directly to Saved Messages.
    pub async fn send_to_self(&self, text: &str) -> Result<(), InvocationError> {
        let req = tl::functions::messages::SendMessage {
            no_webpage:               false,
            silent:                   false,
            background:               false,
            clear_draft:              false,
            noforwards:               false,
            update_stickersets_order: false,
            invert_media:             false,
            allow_paid_floodskip:     false,
            peer:                     tl::enums::InputPeer::PeerSelf,
            reply_to:                 None,
            message:                  text.to_string(),
            random_id:                random_i64(),
            reply_markup:             None,
            entities:                 None,
            schedule_date:            None,
            schedule_repeat_period:   None,
            send_as:                  None,
            quick_reply_shortcut:     None,
            effect:                   None,
            allow_paid_stars:         None,
            suggested_post:           None,
        };
        self.rpc_write(&req).await
    }

    /// Edit an existing message.
    pub async fn edit_message(
        &self,
        peer:       tl::enums::Peer,
        message_id: i32,
        new_text:   &str,
    ) -> Result<(), InvocationError> {
        let input_peer = self.inner.peer_cache.lock().await.peer_to_input(&peer);
        let req = tl::functions::messages::EditMessage {
            no_webpage:    false,
            invert_media:  false,
            peer:          input_peer,
            id:            message_id,
            message:       Some(new_text.to_string()),
            media:         None,
            reply_markup:  None,
            entities:      None,
            schedule_date: None,
            schedule_repeat_period: None,
            quick_reply_shortcut_id: None,
        };
        self.rpc_write(&req).await
    }

    /// Forward messages from `source` to `destination`.
    pub async fn forward_messages(
        &self,
        destination: tl::enums::Peer,
        message_ids: &[i32],
        source:      tl::enums::Peer,
    ) -> Result<(), InvocationError> {
        let cache = self.inner.peer_cache.lock().await;
        let to_peer   = cache.peer_to_input(&destination);
        let from_peer = cache.peer_to_input(&source);
        drop(cache);

        let req = tl::functions::messages::ForwardMessages {
            silent:            false,
            background:        false,
            with_my_score:     false,
            drop_author:       false,
            drop_media_captions: false,
            noforwards:        false,
            from_peer:         from_peer,
            id:                message_ids.to_vec(),
            random_id:         (0..message_ids.len()).map(|_| random_i64()).collect(),
            to_peer:           to_peer,
            top_msg_id:        None,
            reply_to:          None,
            schedule_date:     None,
            schedule_repeat_period: None,
            send_as:           None,
            quick_reply_shortcut: None,
            effect:            None,
            video_timestamp:   None,
            allow_paid_stars:  None,
            allow_paid_floodskip: false,
            suggested_post:    None,
        };
        self.rpc_write(&req).await
    }

    /// Delete messages by ID.
    pub async fn delete_messages(&self, message_ids: Vec<i32>, revoke: bool) -> Result<(), InvocationError> {
        let req = tl::functions::messages::DeleteMessages { revoke, id: message_ids };
        self.rpc_write(&req).await
    }

    /// Get messages by their IDs from a peer.
    pub async fn get_messages_by_id(
        &self,
        peer: tl::enums::Peer,
        ids:  &[i32],
    ) -> Result<Vec<update::IncomingMessage>, InvocationError> {
        let input_peer = self.inner.peer_cache.lock().await.peer_to_input(&peer);
        let id_list: Vec<tl::enums::InputMessage> = ids.iter()
            .map(|&id| tl::enums::InputMessage::Id(tl::types::InputMessageId { id }))
            .collect();
        let req  = tl::functions::channels::GetMessages {
            channel: match &input_peer {
                tl::enums::InputPeer::Channel(c) =>
                    tl::enums::InputChannel::InputChannel(tl::types::InputChannel {
                        channel_id: c.channel_id, access_hash: c.access_hash
                    }),
                _ => return self.get_messages_user(input_peer, id_list).await,
            },
            id: id_list,
        };
        let body    = self.rpc_call_raw(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        let msgs = match tl::enums::messages::Messages::deserialize(&mut cur)? {
            tl::enums::messages::Messages::Messages(m) => m.messages,
            tl::enums::messages::Messages::Slice(m)    => m.messages,
            tl::enums::messages::Messages::ChannelMessages(m) => m.messages,
            tl::enums::messages::Messages::NotModified(_) => vec![],
        };
        Ok(msgs.into_iter().map(update::IncomingMessage::from_raw).collect())
    }

    async fn get_messages_user(
        &self,
        _peer: tl::enums::InputPeer,
        ids:   Vec<tl::enums::InputMessage>,
    ) -> Result<Vec<update::IncomingMessage>, InvocationError> {
        let req = tl::functions::messages::GetMessages { id: ids };
        let body    = self.rpc_call_raw(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        let msgs = match tl::enums::messages::Messages::deserialize(&mut cur)? {
            tl::enums::messages::Messages::Messages(m) => m.messages,
            tl::enums::messages::Messages::Slice(m)    => m.messages,
            tl::enums::messages::Messages::ChannelMessages(m) => m.messages,
            tl::enums::messages::Messages::NotModified(_) => vec![],
        };
        Ok(msgs.into_iter().map(update::IncomingMessage::from_raw).collect())
    }

    /// Get the pinned message in a chat.
    pub async fn get_pinned_message(
        &self,
        peer: tl::enums::Peer,
    ) -> Result<Option<update::IncomingMessage>, InvocationError> {
        let input_peer = self.inner.peer_cache.lock().await.peer_to_input(&peer);
        let req = tl::functions::messages::Search {
            peer: input_peer,
            q: String::new(),
            from_id: None,
            saved_peer_id: None,
            saved_reaction: None,
            top_msg_id: None,
            filter: tl::enums::MessagesFilter::InputMessagesFilterPinned,
            min_date: 0,
            max_date: 0,
            offset_id: 0,
            add_offset: 0,
            limit: 1,
            max_id: 0,
            min_id: 0,
            hash: 0,
        };
        let body    = self.rpc_call_raw(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        let msgs = match tl::enums::messages::Messages::deserialize(&mut cur)? {
            tl::enums::messages::Messages::Messages(m) => m.messages,
            tl::enums::messages::Messages::Slice(m)    => m.messages,
            tl::enums::messages::Messages::ChannelMessages(m) => m.messages,
            tl::enums::messages::Messages::NotModified(_) => vec![],
        };
        Ok(msgs.into_iter().next().map(update::IncomingMessage::from_raw))
    }

    /// Pin a message in a chat.
    pub async fn pin_message(
        &self,
        peer:       tl::enums::Peer,
        message_id: i32,
        silent:     bool,
        unpin:      bool,
        pm_oneside: bool,
    ) -> Result<(), InvocationError> {
        let input_peer = self.inner.peer_cache.lock().await.peer_to_input(&peer);
        let req = tl::functions::messages::UpdatePinnedMessage {
            silent,
            unpin,
            pm_oneside,
            peer: input_peer,
            id:   message_id,
        };
        self.rpc_write(&req).await
    }

    /// Unpin a specific message.
    pub async fn unpin_message(
        &self,
        peer:       tl::enums::Peer,
        message_id: i32,
    ) -> Result<(), InvocationError> {
        self.pin_message(peer, message_id, true, true, false).await
    }

    /// Unpin all messages in a chat.
    pub async fn unpin_all_messages(&self, peer: tl::enums::Peer) -> Result<(), InvocationError> {
        let input_peer = self.inner.peer_cache.lock().await.peer_to_input(&peer);
        let req = tl::functions::messages::UnpinAllMessages {
            peer:      input_peer,
            top_msg_id: None,
            saved_peer_id: None,
        };
        self.rpc_write(&req).await
    }

    // ── Message search ─────────────────────────────────────────────────────

    /// Search messages in a chat.
    pub async fn search_messages(
        &self,
        peer:  tl::enums::Peer,
        query: &str,
        limit: i32,
    ) -> Result<Vec<update::IncomingMessage>, InvocationError> {
        let input_peer = self.inner.peer_cache.lock().await.peer_to_input(&peer);
        let req = tl::functions::messages::Search {
            peer:         input_peer,
            q:            query.to_string(),
            from_id:      None,
            saved_peer_id: None,
            saved_reaction: None,
            top_msg_id:   None,
            filter:       tl::enums::MessagesFilter::InputMessagesFilterEmpty,
            min_date:     0,
            max_date:     0,
            offset_id:    0,
            add_offset:   0,
            limit,
            max_id:       0,
            min_id:       0,
            hash:         0,
        };
        let body    = self.rpc_call_raw(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        let msgs = match tl::enums::messages::Messages::deserialize(&mut cur)? {
            tl::enums::messages::Messages::Messages(m) => m.messages,
            tl::enums::messages::Messages::Slice(m)    => m.messages,
            tl::enums::messages::Messages::ChannelMessages(m) => m.messages,
            tl::enums::messages::Messages::NotModified(_) => vec![],
        };
        Ok(msgs.into_iter().map(update::IncomingMessage::from_raw).collect())
    }

    /// Search messages globally across all chats.
    pub async fn search_global(
        &self,
        query: &str,
        limit: i32,
    ) -> Result<Vec<update::IncomingMessage>, InvocationError> {
        let req = tl::functions::messages::SearchGlobal {
            broadcasts_only: false,
            groups_only:     false,
            users_only:      false,
            folder_id:       None,
            q:               query.to_string(),
            filter:          tl::enums::MessagesFilter::InputMessagesFilterEmpty,
            min_date:        0,
            max_date:        0,
            offset_rate:     0,
            offset_peer:     tl::enums::InputPeer::Empty,
            offset_id:       0,
            limit,
        };
        let body    = self.rpc_call_raw(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        let msgs = match tl::enums::messages::Messages::deserialize(&mut cur)? {
            tl::enums::messages::Messages::Messages(m) => m.messages,
            tl::enums::messages::Messages::Slice(m)    => m.messages,
            tl::enums::messages::Messages::ChannelMessages(m) => m.messages,
            tl::enums::messages::Messages::NotModified(_) => vec![],
        };
        Ok(msgs.into_iter().map(update::IncomingMessage::from_raw).collect())
    }

    // ── Scheduled messages ─────────────────────────────────────────────────

    /// Retrieve all scheduled messages in a chat.
    ///
    /// Scheduled messages are messages set to be sent at a future time using
    /// [`InputMessage::schedule_date`].  Returns them newest-first.
    ///
    /// # Example
    /// ```rust,no_run
    /// # async fn f(client: layer_client::Client, peer: layer_tl_types::enums::Peer) -> Result<(), Box<dyn std::error::Error>> {
    /// let scheduled = client.get_scheduled_messages(peer).await?;
    /// for msg in &scheduled {
    ///     println!("Scheduled: {:?} at {:?}", msg.text(), msg.date());
    /// }
    /// # Ok(()) }
    /// ```
    pub async fn get_scheduled_messages(
        &self,
        peer: tl::enums::Peer,
    ) -> Result<Vec<update::IncomingMessage>, InvocationError> {
        let input_peer = self.inner.peer_cache.lock().await.peer_to_input(&peer);
        let req = tl::functions::messages::GetScheduledHistory {
            peer: input_peer,
            hash: 0,
        };
        let body    = self.rpc_call_raw(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        let msgs = match tl::enums::messages::Messages::deserialize(&mut cur)? {
            tl::enums::messages::Messages::Messages(m)        => m.messages,
            tl::enums::messages::Messages::Slice(m)           => m.messages,
            tl::enums::messages::Messages::ChannelMessages(m) => m.messages,
            tl::enums::messages::Messages::NotModified(_)     => vec![],
        };
        Ok(msgs.into_iter().map(update::IncomingMessage::from_raw).collect())
    }

    /// Delete one or more scheduled messages by their IDs.
    pub async fn delete_scheduled_messages(
        &self,
        peer: tl::enums::Peer,
        ids:  Vec<i32>,
    ) -> Result<(), InvocationError> {
        let input_peer = self.inner.peer_cache.lock().await.peer_to_input(&peer);
        let req = tl::functions::messages::DeleteScheduledMessages {
            peer: input_peer,
            id:   ids,
        };
        self.rpc_write(&req).await
    }

    // ── Callback / Inline Queries ──────────────────────────────────────────

    pub async fn answer_callback_query(
        &self,
        query_id: i64,
        text:     Option<&str>,
        alert:    bool,
    ) -> Result<bool, InvocationError> {
        let req = tl::functions::messages::SetBotCallbackAnswer {
            alert,
            query_id,
            message:    text.map(|s| s.to_string()),
            url:        None,
            cache_time: 0,
        };
        let body = self.rpc_call_raw(&req).await?;
        Ok(!body.is_empty())
    }

    pub async fn answer_inline_query(
        &self,
        query_id:    i64,
        results:     Vec<tl::enums::InputBotInlineResult>,
        cache_time:  i32,
        is_personal: bool,
        next_offset: Option<String>,
    ) -> Result<bool, InvocationError> {
        let req = tl::functions::messages::SetInlineBotResults {
            gallery:        false,
            private:        is_personal,
            query_id,
            results,
            cache_time,
            next_offset,
            switch_pm:      None,
            switch_webview: None,
        };
        let body = self.rpc_call_raw(&req).await?;
        Ok(!body.is_empty())
    }

    // ── Dialogs ────────────────────────────────────────────────────────────

    /// Fetch up to `limit` dialogs, most recent first. Populates entity/message.
    pub async fn get_dialogs(&self, limit: i32) -> Result<Vec<Dialog>, InvocationError> {
        let req = tl::functions::messages::GetDialogs {
            exclude_pinned: false,
            folder_id:      None,
            offset_date:    0,
            offset_id:      0,
            offset_peer:    tl::enums::InputPeer::Empty,
            limit,
            hash:           0,
        };

        let body    = self.rpc_call_raw(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        let raw = match tl::enums::messages::Dialogs::deserialize(&mut cur)? {
            tl::enums::messages::Dialogs::Dialogs(d) => d,
            tl::enums::messages::Dialogs::Slice(d)   => tl::types::messages::Dialogs {
                dialogs: d.dialogs, messages: d.messages, chats: d.chats, users: d.users,
            },
            tl::enums::messages::Dialogs::NotModified(_) => return Ok(vec![]),
        };

        // Build message map
        let msg_map: HashMap<i32, tl::enums::Message> = raw.messages.into_iter()
            .filter_map(|m| {
                let id = match &m {
                    tl::enums::Message::Message(x) => x.id,
                    tl::enums::Message::Service(x) => x.id,
                    tl::enums::Message::Empty(x)   => x.id,
                };
                Some((id, m))
            })
            .collect();

        // Build user map
        let user_map: HashMap<i64, tl::enums::User> = raw.users.into_iter()
            .filter_map(|u| {
                if let tl::enums::User::User(ref uu) = u { Some((uu.id, u)) } else { None }
            })
            .collect();

        // Build chat map
        let chat_map: HashMap<i64, tl::enums::Chat> = raw.chats.into_iter()
            .filter_map(|c| {
                let id = match &c {
                    tl::enums::Chat::Chat(x)             => x.id,
                    tl::enums::Chat::Forbidden(x)    => x.id,
                    tl::enums::Chat::Channel(x)          => x.id,
                    tl::enums::Chat::ChannelForbidden(x) => x.id,
                    tl::enums::Chat::Empty(x)            => x.id,
                };
                Some((id, c))
            })
            .collect();

        // Cache peers for future access_hash lookups
        {
            let u_list: Vec<tl::enums::User> = user_map.values().cloned().collect();
            let c_list: Vec<tl::enums::Chat> = chat_map.values().cloned().collect();
            self.cache_users_slice(&u_list).await;
            self.cache_chats_slice(&c_list).await;
        }

        let result = raw.dialogs.into_iter().map(|d| {
            let top_id = match &d { tl::enums::Dialog::Dialog(x) => x.top_message, _ => 0 };
            let peer   = match &d { tl::enums::Dialog::Dialog(x) => Some(&x.peer), _ => None };

            let message = msg_map.get(&top_id).cloned();
            let entity = peer.and_then(|p| match p {
                tl::enums::Peer::User(u) => user_map.get(&u.user_id).cloned(),
                _ => None,
            });
            let chat = peer.and_then(|p| match p {
                tl::enums::Peer::Chat(c)    => chat_map.get(&c.chat_id).cloned(),
                tl::enums::Peer::Channel(c) => chat_map.get(&c.channel_id).cloned(),
                _ => None,
            });

            Dialog { raw: d, message, entity, chat }
        }).collect();

        Ok(result)
    }

    /// Internal helper: fetch dialogs with a custom GetDialogs request.
    async fn get_dialogs_raw(
        &self,
        req: tl::functions::messages::GetDialogs,
    ) -> Result<Vec<Dialog>, InvocationError> {
        let body    = self.rpc_call_raw(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        let raw = match tl::enums::messages::Dialogs::deserialize(&mut cur)? {
            tl::enums::messages::Dialogs::Dialogs(d) => d,
            tl::enums::messages::Dialogs::Slice(d)   => tl::types::messages::Dialogs {
                dialogs: d.dialogs, messages: d.messages, chats: d.chats, users: d.users,
            },
            tl::enums::messages::Dialogs::NotModified(_) => return Ok(vec![]),
        };

        let msg_map: HashMap<i32, tl::enums::Message> = raw.messages.into_iter()
            .filter_map(|m| {
                let id = match &m {
                    tl::enums::Message::Message(x) => x.id,
                    tl::enums::Message::Service(x) => x.id,
                    tl::enums::Message::Empty(x)   => x.id,
                };
                Some((id, m))
            })
            .collect();

        let user_map: HashMap<i64, tl::enums::User> = raw.users.into_iter()
            .filter_map(|u| {
                if let tl::enums::User::User(ref uu) = u { Some((uu.id, u)) } else { None }
            })
            .collect();

        let chat_map: HashMap<i64, tl::enums::Chat> = raw.chats.into_iter()
            .filter_map(|c| {
                let id = match &c {
                    tl::enums::Chat::Chat(x)             => x.id,
                    tl::enums::Chat::Forbidden(x)    => x.id,
                    tl::enums::Chat::Channel(x)          => x.id,
                    tl::enums::Chat::ChannelForbidden(x) => x.id,
                    tl::enums::Chat::Empty(x)            => x.id,
                };
                Some((id, c))
            })
            .collect();

        {
            let u_list: Vec<tl::enums::User> = user_map.values().cloned().collect();
            let c_list: Vec<tl::enums::Chat> = chat_map.values().cloned().collect();
            self.cache_users_slice(&u_list).await;
            self.cache_chats_slice(&c_list).await;
        }

        let result = raw.dialogs.into_iter().map(|d| {
            let top_id = match &d { tl::enums::Dialog::Dialog(x) => x.top_message, _ => 0 };
            let peer   = match &d { tl::enums::Dialog::Dialog(x) => Some(&x.peer), _ => None };

            let message = msg_map.get(&top_id).cloned();
            let entity = peer.and_then(|p| match p {
                tl::enums::Peer::User(u) => user_map.get(&u.user_id).cloned(),
                _ => None,
            });
            let chat = peer.and_then(|p| match p {
                tl::enums::Peer::Chat(c)    => chat_map.get(&c.chat_id).cloned(),
                tl::enums::Peer::Channel(c) => chat_map.get(&c.channel_id).cloned(),
                _ => None,
            });

            Dialog { raw: d, message, entity, chat }
        }).collect();

        Ok(result)
    }
    pub async fn delete_dialog(&self, peer: tl::enums::Peer) -> Result<(), InvocationError> {
        let input_peer = self.inner.peer_cache.lock().await.peer_to_input(&peer);
        let req = tl::functions::messages::DeleteHistory {
            just_clear:  false,
            revoke:      false,
            peer:        input_peer,
            max_id:      0,
            min_date:    None,
            max_date:    None,
        };
        self.rpc_write(&req).await
    }

    /// Mark all messages in a chat as read.
    pub async fn mark_as_read(&self, peer: tl::enums::Peer) -> Result<(), InvocationError> {
        let input_peer = self.inner.peer_cache.lock().await.peer_to_input(&peer);
        match &input_peer {
            tl::enums::InputPeer::Channel(c) => {
                let req = tl::functions::channels::ReadHistory {
                    channel: tl::enums::InputChannel::InputChannel(tl::types::InputChannel {
                        channel_id: c.channel_id, access_hash: c.access_hash,
                    }),
                    max_id: 0,
                };
                self.rpc_call_raw(&req).await?;
            }
            _ => {
                let req = tl::functions::messages::ReadHistory { peer: input_peer, max_id: 0 };
                self.rpc_call_raw(&req).await?;
            }
        }
        Ok(())
    }

    /// Clear unread mention markers.
    pub async fn clear_mentions(&self, peer: tl::enums::Peer) -> Result<(), InvocationError> {
        let input_peer = self.inner.peer_cache.lock().await.peer_to_input(&peer);
        let req = tl::functions::messages::ReadMentions { peer: input_peer, top_msg_id: None };
        self.rpc_write(&req).await
    }

    // ── Chat actions (typing, etc) ─────────────────────────────────────────

    /// Send a chat action (typing indicator, uploading photo, etc).
    ///
    /// For "typing" use `tl::enums::SendMessageAction::Typing`.
    pub async fn send_chat_action(
        &self,
        peer:   tl::enums::Peer,
        action: tl::enums::SendMessageAction,
    ) -> Result<(), InvocationError> {
        let input_peer = self.inner.peer_cache.lock().await.peer_to_input(&peer);
        let req = tl::functions::messages::SetTyping {
            peer: input_peer,
            top_msg_id: None,
            action,
        };
        self.rpc_write(&req).await
    }

    // ── Join / invite links ────────────────────────────────────────────────

    /// Join a public chat or channel by username/peer.
    pub async fn join_chat(&self, peer: tl::enums::Peer) -> Result<(), InvocationError> {
        let input_peer = self.inner.peer_cache.lock().await.peer_to_input(&peer);
        match input_peer {
            tl::enums::InputPeer::Channel(c) => {
                let req = tl::functions::channels::JoinChannel {
                    channel: tl::enums::InputChannel::InputChannel(tl::types::InputChannel {
                        channel_id: c.channel_id, access_hash: c.access_hash,
                    }),
                };
                self.rpc_call_raw(&req).await?;
            }
            tl::enums::InputPeer::Chat(c) => {
                let req = tl::functions::messages::AddChatUser {
                    chat_id:  c.chat_id,
                    user_id:  tl::enums::InputUser::UserSelf,
                    fwd_limit: 0,
                };
                self.rpc_call_raw(&req).await?;
            }
            _ => return Err(InvocationError::Deserialize("cannot join this peer type".into())),
        }
        Ok(())
    }

    /// Accept and join via an invite link.
    pub async fn accept_invite_link(&self, link: &str) -> Result<(), InvocationError> {
        let hash = Self::parse_invite_hash(link)
            .ok_or_else(|| InvocationError::Deserialize(format!("invalid invite link: {link}")))?;
        let req = tl::functions::messages::ImportChatInvite { hash: hash.to_string() };
        self.rpc_write(&req).await
    }

    /// Extract hash from `https://t.me/+HASH` or `https://t.me/joinchat/HASH`.
    pub fn parse_invite_hash(link: &str) -> Option<&str> {
        if let Some(pos) = link.find("/+") {
            return Some(&link[pos + 2..]);
        }
        if let Some(pos) = link.find("/joinchat/") {
            return Some(&link[pos + 10..]);
        }
        None
    }

    // ── Message history (paginated) ────────────────────────────────────────

    /// Fetch a page of messages from a peer's history.
    pub async fn get_messages(
        &self,
        peer:      tl::enums::InputPeer,
        limit:     i32,
        offset_id: i32,
    ) -> Result<Vec<update::IncomingMessage>, InvocationError> {
        let req = tl::functions::messages::GetHistory {
            peer, offset_id, offset_date: 0, add_offset: 0,
            limit, max_id: 0, min_id: 0, hash: 0,
        };
        let body    = self.rpc_call_raw(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        let msgs = match tl::enums::messages::Messages::deserialize(&mut cur)? {
            tl::enums::messages::Messages::Messages(m) => m.messages,
            tl::enums::messages::Messages::Slice(m)    => m.messages,
            tl::enums::messages::Messages::ChannelMessages(m) => m.messages,
            tl::enums::messages::Messages::NotModified(_) => vec![],
        };
        Ok(msgs.into_iter().map(update::IncomingMessage::from_raw).collect())
    }

    // ── Peer resolution ────────────────────────────────────────────────────

    /// Resolve a peer string to a [`tl::enums::Peer`].
    pub async fn resolve_peer(
        &self,
        peer: &str,
    ) -> Result<tl::enums::Peer, InvocationError> {
        match peer.trim() {
            "me" | "self" => Ok(tl::enums::Peer::User(tl::types::PeerUser { user_id: 0 })),
            username if username.starts_with('@') => {
                self.resolve_username(&username[1..]).await
            }
            id_str => {
                if let Ok(id) = id_str.parse::<i64>() {
                    Ok(tl::enums::Peer::User(tl::types::PeerUser { user_id: id }))
                } else {
                    Err(InvocationError::Deserialize(format!("cannot resolve peer: {peer}")))
                }
            }
        }
    }

    async fn resolve_username(&self, username: &str) -> Result<tl::enums::Peer, InvocationError> {
        let req  = tl::functions::contacts::ResolveUsername {
            username: username.to_string(), referer: None,
        };
        let body = self.rpc_call_raw(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        let resolved = match tl::enums::contacts::ResolvedPeer::deserialize(&mut cur)? {
            tl::enums::contacts::ResolvedPeer::ResolvedPeer(r) => r,
        };
        // Cache users and chats from the resolution
        self.cache_users_slice(&resolved.users).await;
        self.cache_chats_slice(&resolved.chats).await;
        Ok(resolved.peer)
    }

    // ── Raw invoke ─────────────────────────────────────────────────────────

    /// Invoke any TL function directly, handling flood-wait retries.
    pub async fn invoke<R: RemoteCall>(&self, req: &R) -> Result<R::Return, InvocationError> {
        let body = self.rpc_call_raw(req).await?;
        let mut cur = Cursor::from_slice(&body);
        R::Return::deserialize(&mut cur).map_err(Into::into)
    }

    async fn rpc_call_raw<R: RemoteCall>(&self, req: &R) -> Result<Vec<u8>, InvocationError> {
        let mut fail_count   = NonZeroU32::new(1).unwrap();
        let mut slept_so_far = Duration::default();
        loop {
            match self.do_rpc_call(req).await {
                Ok(body) => return Ok(body),
                Err(e) => {
                    let ctx = RetryContext { fail_count, slept_so_far, error: e };
                    match self.inner.retry_policy.should_retry(&ctx) {
                        ControlFlow::Continue(delay) => {
                            sleep(delay).await;
                            slept_so_far += delay;
                            fail_count = fail_count.saturating_add(1);
                        }
                        ControlFlow::Break(()) => return Err(ctx.error),
                    }
                }
            }
        }
    }

    async fn do_rpc_call<R: RemoteCall>(&self, req: &R) -> Result<Vec<u8>, InvocationError> {
        let mut conn = self.inner.conn.lock().await;
        conn.rpc_call(req).await
    }

    /// Like `rpc_call_raw` but for write RPCs whose TL return type is `Updates`.
    /// Accepts either a normal payload or an `Updates` frame as success, so we
    /// don't hang when Telegram sends back an `updateShort` instead of a full result.
    async fn rpc_write<S: tl::Serializable>(&self, req: &S) -> Result<(), InvocationError> {
        let mut fail_count   = NonZeroU32::new(1).unwrap();
        let mut slept_so_far = Duration::default();
        loop {
            let result = {
                let mut conn = self.inner.conn.lock().await;
                conn.rpc_call_ack(req).await
            };
            match result {
                Ok(()) => return Ok(()),
                Err(e) => {
                    let ctx = RetryContext { fail_count, slept_so_far, error: e };
                    match self.inner.retry_policy.should_retry(&ctx) {
                        ControlFlow::Continue(delay) => {
                            sleep(delay).await;
                            slept_so_far += delay;
                            fail_count = fail_count.saturating_add(1);
                        }
                        ControlFlow::Break(()) => return Err(ctx.error),
                    }
                }
            }
        }
    }

    // ── initConnection ─────────────────────────────────────────────────────

    async fn init_connection(&self) -> Result<(), InvocationError> {
        use tl::functions::{InvokeWithLayer, InitConnection, help::GetConfig};
        let req = InvokeWithLayer {
            layer: tl::LAYER,
            query: InitConnection {
                api_id:           self.inner.api_id,
                device_model:     "Linux".to_string(),
                system_version:   "1.0".to_string(),
                app_version:      env!("CARGO_PKG_VERSION").to_string(),
                system_lang_code: "en".to_string(),
                lang_pack:        "".to_string(),
                lang_code:        "en".to_string(),
                proxy:            None,
                params:           None,
                query:            GetConfig {},
            },
        };

        let body = {
            let mut conn = self.inner.conn.lock().await;
            conn.rpc_call_serializable(&req).await?
        };

        let mut cur = Cursor::from_slice(&body);
        if let Ok(tl::enums::Config::Config(cfg)) = tl::enums::Config::deserialize(&mut cur) {
            let allow_ipv6 = self.inner.allow_ipv6;
            let mut opts = self.inner.dc_options.lock().await;
            for opt in &cfg.dc_options {
                let tl::enums::DcOption::DcOption(o) = opt;
                if o.media_only || o.cdn || o.tcpo_only { continue; }
                if o.ipv6 && !allow_ipv6 { continue; }
                let addr = format!("{}:{}", o.ip_address, o.port);
                let entry = opts.entry(o.id).or_insert_with(|| DcEntry {
                    dc_id: o.id, addr: addr.clone(),
                    auth_key: None, first_salt: 0, time_offset: 0,
                });
                entry.addr = addr;
            }
            log::info!("[layer] initConnection ✓  ({} DCs, ipv6={})", cfg.dc_options.len(), allow_ipv6);
        }
        Ok(())
    }

    // ── DC migration ───────────────────────────────────────────────────────

    async fn migrate_to(&self, new_dc_id: i32) -> Result<(), InvocationError> {
        let addr = {
            let opts = self.inner.dc_options.lock().await;
            opts.get(&new_dc_id).map(|e| e.addr.clone())
                .unwrap_or_else(|| "149.154.167.51:443".to_string())
        };
        log::info!("[layer] Migrating to DC{new_dc_id} ({addr}) …");

        let saved_key = {
            let opts = self.inner.dc_options.lock().await;
            opts.get(&new_dc_id).and_then(|e| e.auth_key)
        };

        let socks5    = self.inner.socks5.clone();
        let transport = self.inner.transport.clone();
        let conn = if let Some(key) = saved_key {
            Connection::connect_with_key(&addr, key, 0, 0, socks5.as_ref(), &transport).await?
        } else {
            Connection::connect_raw(&addr, socks5.as_ref(), &transport).await?
        };

        let new_key = conn.auth_key_bytes();
        {
            let mut opts = self.inner.dc_options.lock().await;
            let entry = opts.entry(new_dc_id).or_insert_with(|| DcEntry {
                dc_id: new_dc_id, addr: addr.clone(),
                auth_key: None, first_salt: 0, time_offset: 0,
            });
            entry.auth_key = Some(new_key);
        }

        *self.inner.conn.lock().await = conn;
        *self.inner.home_dc_id.lock().await = new_dc_id;
        self.init_connection().await?;
        log::info!("[layer] Now on DC{new_dc_id} ✓");
        Ok(())
    }

    // ── Cache helpers ──────────────────────────────────────────────────────

    async fn cache_user(&self, user: &tl::enums::User) {
        self.inner.peer_cache.lock().await.cache_user(user);
    }

    async fn cache_users_slice(&self, users: &[tl::enums::User]) {
        let mut cache = self.inner.peer_cache.lock().await;
        cache.cache_users(users);
    }

    async fn cache_chats_slice(&self, chats: &[tl::enums::Chat]) {
        let mut cache = self.inner.peer_cache.lock().await;
        cache.cache_chats(chats);
    }

    // Public versions used by sub-modules (media.rs, participants.rs, pts.rs)
    #[doc(hidden)]
    pub async fn cache_users_slice_pub(&self, users: &[tl::enums::User]) {
        self.cache_users_slice(users).await;
    }

    #[doc(hidden)]
    pub async fn cache_chats_slice_pub(&self, chats: &[tl::enums::Chat]) {
        self.cache_chats_slice(chats).await;
    }

    /// Public RPC call for use by sub-modules.
    #[doc(hidden)]
    pub async fn rpc_call_raw_pub<R: layer_tl_types::RemoteCall>(&self, req: &R) -> Result<Vec<u8>, InvocationError> {
        self.rpc_call_raw(req).await
    }

    // ── Paginated dialog iterator ──────────────────────────────────────────

    /// Fetch dialogs page by page.
    ///
    /// Returns a [`DialogIter`] that can be advanced with [`DialogIter::next`].
    /// This lets you page through all dialogs without loading them all at once.
    ///
    /// # Example
    /// ```rust,no_run
    /// # async fn f(client: layer_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let mut iter = client.iter_dialogs();
    /// while let Some(dialog) = iter.next(&client).await? {
    ///     println!("{}", dialog.title());
    /// }
    /// # Ok(()) }
    /// ```
    pub fn iter_dialogs(&self) -> DialogIter {
        DialogIter {
            offset_date: 0,
            offset_id:   0,
            offset_peer: tl::enums::InputPeer::Empty,
            done:        false,
            buffer:      VecDeque::new(),
        }
    }

    /// Fetch messages from a peer, page by page.
    ///
    /// Returns a [`MessageIter`] that can be advanced with [`MessageIter::next`].
    ///
    /// # Example
    /// ```rust,no_run
    /// # async fn f(client: layer_client::Client, peer: layer_tl_types::enums::Peer) -> Result<(), Box<dyn std::error::Error>> {
    /// let mut iter = client.iter_messages(peer);
    /// while let Some(msg) = iter.next(&client).await? {
    ///     println!("{:?}", msg.text());
    /// }
    /// # Ok(()) }
    /// ```
    pub fn iter_messages(&self, peer: tl::enums::Peer) -> MessageIter {
        MessageIter {
            peer,
            offset_id: 0,
            done:      false,
            buffer:    VecDeque::new(),
        }
    }

    // ── resolve_peer helper returning Result on unknown hash ───────────────

    /// Try to resolve a peer to InputPeer, returning an error if the access_hash
    /// is unknown (i.e. the peer has not been seen in any prior API call).
    pub async fn resolve_to_input_peer(
        &self,
        peer: &tl::enums::Peer,
    ) -> Result<tl::enums::InputPeer, InvocationError> {
        let cache = self.inner.peer_cache.lock().await;
        match peer {
            tl::enums::Peer::User(u) => {
                if u.user_id == 0 {
                    return Ok(tl::enums::InputPeer::PeerSelf);
                }
                match cache.users.get(&u.user_id) {
                    Some(&hash) => Ok(tl::enums::InputPeer::User(tl::types::InputPeerUser {
                        user_id: u.user_id, access_hash: hash,
                    })),
                    None => Err(InvocationError::Deserialize(format!(
                        "access_hash unknown for user {}; resolve via username first", u.user_id
                    ))),
                }
            }
            tl::enums::Peer::Chat(c) => {
                Ok(tl::enums::InputPeer::Chat(tl::types::InputPeerChat { chat_id: c.chat_id }))
            }
            tl::enums::Peer::Channel(c) => {
                match cache.channels.get(&c.channel_id) {
                    Some(&hash) => Ok(tl::enums::InputPeer::Channel(tl::types::InputPeerChannel {
                        channel_id: c.channel_id, access_hash: hash,
                    })),
                    None => Err(InvocationError::Deserialize(format!(
                        "access_hash unknown for channel {}; resolve via username first", c.channel_id
                    ))),
                }
            }
        }
    }

    // ── Multi-DC pool ──────────────────────────────────────────────────────

    /// Invoke a request on a specific DC, using the pool.
    ///
    /// If the target DC has no auth key yet, one is acquired via DH and then
    /// authorized via `auth.exportAuthorization` / `auth.importAuthorization`
    /// so the worker DC can serve user-account requests too.
    pub async fn invoke_on_dc<R: RemoteCall>(
        &self,
        dc_id: i32,
        req:   &R,
    ) -> Result<R::Return, InvocationError> {
        let body = self.rpc_on_dc_raw(dc_id, req).await?;
        let mut cur = Cursor::from_slice(&body);
        R::Return::deserialize(&mut cur).map_err(Into::into)
    }

    /// Raw RPC call routed to `dc_id`, exporting auth if needed.
    async fn rpc_on_dc_raw<R: RemoteCall>(
        &self,
        dc_id: i32,
        req:   &R,
    ) -> Result<Vec<u8>, InvocationError> {
        // Check if we need to open a new connection for this DC
        let needs_new = {
            let pool = self.inner.dc_pool.lock().await;
            !pool.has_connection(dc_id)
        };

        if needs_new {
            let addr = {
                let opts = self.inner.dc_options.lock().await;
                opts.get(&dc_id).map(|e| e.addr.clone())
                    .ok_or_else(|| InvocationError::Deserialize(format!("unknown DC{dc_id}")))?
            };

            let socks5    = self.inner.socks5.clone();
            let transport = self.inner.transport.clone();
            let saved_key = {
                let opts = self.inner.dc_options.lock().await;
                opts.get(&dc_id).and_then(|e| e.auth_key)
            };

            let dc_conn = if let Some(key) = saved_key {
                dc_pool::DcConnection::connect_with_key(&addr, key, 0, 0, socks5.as_ref(), &transport).await?
            } else {
                let conn = dc_pool::DcConnection::connect_raw(&addr, socks5.as_ref(), &transport).await?;
                // Export auth from home DC and import into worker DC
                let home_dc_id = *self.inner.home_dc_id.lock().await;
                if dc_id != home_dc_id {
                    if let Err(e) = self.export_import_auth(dc_id, &conn).await {
                        log::warn!("[layer] Auth export/import for DC{dc_id} failed: {e}");
                    }
                }
                conn
            };

            let key = dc_conn.auth_key_bytes();
            {
                let mut opts = self.inner.dc_options.lock().await;
                if let Some(e) = opts.get_mut(&dc_id) {
                    e.auth_key = Some(key);
                }
            }
            self.inner.dc_pool.lock().await.insert(dc_id, dc_conn);
        }

        let dc_entries: Vec<DcEntry> = self.inner.dc_options.lock().await.values().cloned().collect();
        self.inner.dc_pool.lock().await.invoke_on_dc(dc_id, &dc_entries, req).await
    }

    /// Export authorization from the home DC and import it into `dc_id`.
    async fn export_import_auth(
        &self,
        dc_id:   i32,
        _dc_conn: &dc_pool::DcConnection, // reserved for future direct import
    ) -> Result<(), InvocationError> {
        // Export from home DC
        let export_req = tl::functions::auth::ExportAuthorization { dc_id };
        let body    = self.rpc_call_raw(&export_req).await?;
        let mut cur = Cursor::from_slice(&body);
        let exported = match tl::enums::auth::ExportedAuthorization::deserialize(&mut cur)? {
            tl::enums::auth::ExportedAuthorization::ExportedAuthorization(e) => e,
        };

        // Import into the target DC via the pool
        let import_req = tl::functions::auth::ImportAuthorization {
            id:    exported.id,
            bytes: exported.bytes,
        };
        let dc_entries: Vec<DcEntry> = self.inner.dc_options.lock().await.values().cloned().collect();
        self.inner.dc_pool.lock().await.invoke_on_dc(dc_id, &dc_entries, &import_req).await?;
        log::info!("[layer] Auth exported+imported to DC{dc_id} ✓");
        Ok(())
    }

    // ── Private helpers ────────────────────────────────────────────────────

    async fn get_password_info(&self) -> Result<PasswordToken, InvocationError> {
        let body    = self.rpc_call_raw(&tl::functions::account::GetPassword {}).await?;
        let mut cur = Cursor::from_slice(&body);
        let pw = match tl::enums::account::Password::deserialize(&mut cur)? {
            tl::enums::account::Password::Password(p) => p,
        };
        Ok(PasswordToken { password: pw })
    }

    fn make_send_code_req(&self, phone: &str) -> tl::functions::auth::SendCode {
        tl::functions::auth::SendCode {
            phone_number: phone.to_string(),
            api_id:       self.inner.api_id,
            api_hash:     self.inner.api_hash.clone(),
            settings:     tl::enums::CodeSettings::CodeSettings(
                tl::types::CodeSettings {
                    allow_flashcall: false, current_number: false, allow_app_hash: false,
                    allow_missed_call: false, allow_firebase: false, unknown_number: false,
                    logout_tokens: None, token: None, app_sandbox: None,
                },
            ),
        }
    }

    fn extract_user_name(user: &tl::enums::User) -> String {
        match user {
            tl::enums::User::User(u) => {
                format!("{} {}",
                    u.first_name.as_deref().unwrap_or(""),
                    u.last_name.as_deref().unwrap_or(""))
                    .trim().to_string()
            }
            tl::enums::User::Empty(_) => "(unknown)".into(),
        }
    }

    fn extract_password_params(
        algo: &tl::enums::PasswordKdfAlgo,
    ) -> Result<(&[u8], &[u8], &[u8], i32), InvocationError> {
        match algo {
            tl::enums::PasswordKdfAlgo::Sha256Sha256Pbkdf2Hmacsha512iter100000Sha256ModPow(a) => {
                Ok((&a.salt1, &a.salt2, &a.p, a.g))
            }
            _ => Err(InvocationError::Deserialize("unsupported password KDF algo".into())),
        }
    }
}

// ─── Paginated iterators ──────────────────────────────────────────────────────

/// Cursor-based iterator over dialogs. Created by [`Client::iter_dialogs`].
pub struct DialogIter {
    offset_date: i32,
    offset_id:   i32,
    offset_peer: tl::enums::InputPeer,
    done:        bool,
    buffer:      VecDeque<Dialog>,
}

impl DialogIter {
    const PAGE_SIZE: i32 = 100;

    /// Fetch the next dialog. Returns `None` when all dialogs have been yielded.
    pub async fn next(&mut self, client: &Client) -> Result<Option<Dialog>, InvocationError> {
        if let Some(d) = self.buffer.pop_front() { return Ok(Some(d)); }
        if self.done { return Ok(None); }

        let req = tl::functions::messages::GetDialogs {
            exclude_pinned: false,
            folder_id:      None,
            offset_date:    self.offset_date,
            offset_id:      self.offset_id,
            offset_peer:    self.offset_peer.clone(),
            limit:          Self::PAGE_SIZE,
            hash:           0,
        };

        let dialogs = client.get_dialogs_raw(req).await?;
        if dialogs.is_empty() || dialogs.len() < Self::PAGE_SIZE as usize {
            self.done = true;
        }

        // Prepare cursor for next page
        if let Some(last) = dialogs.last() {
            self.offset_date = last.message.as_ref().map(|m| match m {
                tl::enums::Message::Message(x) => x.date,
                tl::enums::Message::Service(x) => x.date,
                _ => 0,
            }).unwrap_or(0);
            self.offset_id = last.top_message();
            if let Some(peer) = last.peer() {
                self.offset_peer = client.inner.peer_cache.lock().await.peer_to_input(peer);
            }
        }

        self.buffer.extend(dialogs);
        Ok(self.buffer.pop_front())
    }
}

/// Cursor-based iterator over message history. Created by [`Client::iter_messages`].
pub struct MessageIter {
    peer:      tl::enums::Peer,
    offset_id: i32,
    done:      bool,
    buffer:    VecDeque<update::IncomingMessage>,
}

impl MessageIter {
    const PAGE_SIZE: i32 = 100;

    /// Fetch the next message (newest first). Returns `None` when all messages have been yielded.
    pub async fn next(&mut self, client: &Client) -> Result<Option<update::IncomingMessage>, InvocationError> {
        if let Some(m) = self.buffer.pop_front() { return Ok(Some(m)); }
        if self.done { return Ok(None); }

        let input_peer = client.inner.peer_cache.lock().await.peer_to_input(&self.peer);
        let page = client.get_messages(input_peer, Self::PAGE_SIZE, self.offset_id).await?;

        if page.is_empty() || page.len() < Self::PAGE_SIZE as usize {
            self.done = true;
        }
        if let Some(last) = page.last() {
            self.offset_id = last.id();
        }

        self.buffer.extend(page);
        Ok(self.buffer.pop_front())
    }
}

// ─── Public random helper (used by media.rs) ──────────────────────────────────

/// Public wrapper for `random_i64` used by sub-modules.
#[doc(hidden)]
pub fn random_i64_pub() -> i64 { random_i64() }

// ─── Connection ───────────────────────────────────────────────────────────────

/// How framing bytes are sent/received on a connection.
enum FrameKind {
    Abridged,
    Intermediate,
    #[allow(dead_code)]
    Full { send_seqno: u32, recv_seqno: u32 },
}

struct Connection {
    stream:     TcpStream,
    enc:        EncryptedSession,
    frame_kind: FrameKind,
}

impl Connection {
    /// Open a TCP stream, optionally via SOCKS5, and apply transport init bytes.
    async fn open_stream(
        addr:      &str,
        socks5:    Option<&crate::socks5::Socks5Config>,
        transport: &TransportKind,
    ) -> Result<(TcpStream, FrameKind), InvocationError> {
        let stream = match socks5 {
            Some(proxy) => proxy.connect(addr).await?,
            None        => TcpStream::connect(addr).await?,
        };
        Self::apply_transport_init(stream, transport).await
    }

    /// Send the transport init bytes and return the stream + FrameKind.
    async fn apply_transport_init(
        mut stream: TcpStream,
        transport:  &TransportKind,
    ) -> Result<(TcpStream, FrameKind), InvocationError> {
        match transport {
            TransportKind::Abridged => {
                stream.write_all(&[0xef]).await?;
                Ok((stream, FrameKind::Abridged))
            }
            TransportKind::Intermediate => {
                stream.write_all(&[0xee, 0xee, 0xee, 0xee]).await?;
                Ok((stream, FrameKind::Intermediate))
            }
            TransportKind::Full => {
                // Full transport has no init byte
                Ok((stream, FrameKind::Full { send_seqno: 0, recv_seqno: 0 }))
            }
            TransportKind::Obfuscated { secret } => {
                // For obfuscated we do the full handshake inside open_obfuscated,
                // then wrap back in a plain TcpStream via into_inner.
                // Since ObfuscatedStream is a different type we reuse the Abridged
                // frame logic internally — the encryption layer handles everything.
                //
                // Implementation note: We convert to Abridged after the handshake
                // because ObfuscatedStream internally already uses Abridged framing
                // with XOR applied on top.  The outer Connection just sends raw bytes.
                let mut nonce = [0u8; 64];
                getrandom::getrandom(&mut nonce).map_err(|_| InvocationError::Deserialize("getrandom".into()))?;
                // Write obfuscated handshake header
                let (enc_key, enc_iv, _dec_key, _dec_iv) = crate::transport_obfuscated::derive_keys(&nonce, secret.as_ref());
                let mut enc_cipher = crate::transport_obfuscated::ObfCipher::new(enc_key, enc_iv);
                // Stamp protocol tag into nonce[56..60]
                let mut handshake = nonce;
                handshake[56] = 0xef; handshake[57] = 0xef;
                handshake[58] = 0xef; handshake[59] = 0xef;
                enc_cipher.apply(&mut handshake[56..]);
                stream.write_all(&handshake).await?;
                Ok((stream, FrameKind::Abridged))
            }
        }
    }

    async fn connect_raw(
        addr:      &str,
        socks5:    Option<&crate::socks5::Socks5Config>,
        transport: &TransportKind,
    ) -> Result<Self, InvocationError> {
        log::info!("[layer] Connecting to {addr} (DH) …");

        // Wrap the entire DH handshake in a timeout so a silent server
        // response (e.g. a mis-framed transport error) never causes an
        // infinite hang.
        let addr2      = addr.to_string();
        let socks5_c   = socks5.cloned();
        let transport_c = transport.clone();

        let fut = async move {
            let (mut stream, frame_kind) =
                Self::open_stream(&addr2, socks5_c.as_ref(), &transport_c).await?;

            let mut plain = Session::new();

            let (req1, s1) = auth::step1().map_err(|e| InvocationError::Deserialize(e.to_string()))?;
            send_frame(&mut stream, &plain.pack(&req1).to_plaintext_bytes(), &frame_kind).await?;
            let res_pq: tl::enums::ResPq = recv_frame_plain(&mut stream, &frame_kind).await?;

            let (req2, s2) = auth::step2(s1, res_pq).map_err(|e| InvocationError::Deserialize(e.to_string()))?;
            send_frame(&mut stream, &plain.pack(&req2).to_plaintext_bytes(), &frame_kind).await?;
            let dh: tl::enums::ServerDhParams = recv_frame_plain(&mut stream, &frame_kind).await?;

            let (req3, s3) = auth::step3(s2, dh).map_err(|e| InvocationError::Deserialize(e.to_string()))?;
            send_frame(&mut stream, &plain.pack(&req3).to_plaintext_bytes(), &frame_kind).await?;
            let ans: tl::enums::SetClientDhParamsAnswer = recv_frame_plain(&mut stream, &frame_kind).await?;

            let done = auth::finish(s3, ans).map_err(|e| InvocationError::Deserialize(e.to_string()))?;
            log::info!("[layer] DH complete ✓");

            Ok::<Self, InvocationError>(Self {
                stream,
                enc: EncryptedSession::new(done.auth_key, done.first_salt, done.time_offset),
                frame_kind,
            })
        };

        tokio::time::timeout(Duration::from_secs(15), fut)
            .await
            .map_err(|_| InvocationError::Deserialize(
                format!("DH handshake with {addr} timed out after 15 s")
            ))?
    }

    async fn connect_with_key(
        addr:        &str,
        auth_key:    [u8; 256],
        first_salt:  i64,
        time_offset: i32,
        socks5:      Option<&crate::socks5::Socks5Config>,
        transport:   &TransportKind,
    ) -> Result<Self, InvocationError> {
        let addr2       = addr.to_string();
        let socks5_c    = socks5.cloned();
        let transport_c = transport.clone();

        let fut = async move {
            let (stream, frame_kind) =
                Self::open_stream(&addr2, socks5_c.as_ref(), &transport_c).await?;
            Ok::<Self, InvocationError>(Self {
                stream,
                enc: EncryptedSession::new(auth_key, first_salt, time_offset),
                frame_kind,
            })
        };

        tokio::time::timeout(Duration::from_secs(15), fut)
            .await
            .map_err(|_| InvocationError::Deserialize(
                format!("connect_with_key to {addr} timed out after 15 s")
            ))?
    }

    fn auth_key_bytes(&self) -> [u8; 256] { self.enc.auth_key_bytes() }
    fn first_salt(&self)     -> i64         { self.enc.salt }
    fn time_offset(&self)    -> i32         { self.enc.time_offset }

    async fn rpc_call<R: RemoteCall>(&mut self, req: &R) -> Result<Vec<u8>, InvocationError> {
        let wire = self.enc.pack(req);
        send_frame(&mut self.stream, &wire, &self.frame_kind).await?;
        tokio::time::timeout(Duration::from_secs(10), self.recv_rpc())
            .await
            .map_err(|_| InvocationError::Deserialize("rpc_call timed out after 10 s".into()))?
    }

    async fn rpc_call_serializable<S: tl::Serializable>(&mut self, req: &S) -> Result<Vec<u8>, InvocationError> {
        let wire = self.enc.pack_serializable(req);
        send_frame(&mut self.stream, &wire, &self.frame_kind).await?;
        tokio::time::timeout(Duration::from_secs(10), self.recv_rpc())
            .await
            .map_err(|_| InvocationError::Deserialize("rpc_call_serializable timed out after 10 s".into()))?
    }

    /// Like `rpc_call_serializable` but accepts either a Payload OR an Updates
    /// frame as a successful response.  Use this for write RPCs whose return
    /// type in the TL schema is `Updates` — Telegram may respond with an
    /// `updateShort` instead of a full serialized result.
    async fn rpc_call_ack<S: tl::Serializable>(&mut self, req: &S) -> Result<(), InvocationError> {
        let wire = self.enc.pack_serializable(req);
        send_frame(&mut self.stream, &wire, &self.frame_kind).await?;
        tokio::time::timeout(Duration::from_secs(10), self.recv_ack())
            .await
            .map_err(|_| InvocationError::Deserialize("rpc_call_ack timed out after 10 s".into()))?
    }

    async fn recv_ack(&mut self) -> Result<(), InvocationError> {
        loop {
            let mut raw = recv_frame(&mut self.stream, &mut self.frame_kind).await?;
            let msg = self.enc.unpack(&mut raw)
                .map_err(|e| InvocationError::Deserialize(e.to_string()))?;
            if msg.salt != 0 { self.enc.salt = msg.salt; }
            match unwrap_envelope(msg.body)? {
                EnvelopeResult::Payload(_) | EnvelopeResult::Updates(_) => return Ok(()),
                EnvelopeResult::None => {}
            }
        }
    }

    async fn recv_rpc(&mut self) -> Result<Vec<u8>, InvocationError> {
        loop {
            let mut raw = recv_frame(&mut self.stream, &mut self.frame_kind).await?;
            let msg = self.enc.unpack(&mut raw)
                .map_err(|e| InvocationError::Deserialize(e.to_string()))?;
            if msg.salt != 0 { self.enc.salt = msg.salt; }
            match unwrap_envelope(msg.body)? {
                EnvelopeResult::Payload(p)  => return Ok(p),
                EnvelopeResult::Updates(us) => {
                    log::debug!("[layer] {} updates during RPC", us.len());
                }
                EnvelopeResult::None => {}
            }
        }
    }

    async fn recv_once(&mut self) -> Result<Vec<update::Update>, InvocationError> {
        let mut raw = recv_frame(&mut self.stream, &mut self.frame_kind).await?;
        let msg = self.enc.unpack(&mut raw)
            .map_err(|e| InvocationError::Deserialize(e.to_string()))?;
        if msg.salt != 0 { self.enc.salt = msg.salt; }
        match unwrap_envelope(msg.body)? {
            EnvelopeResult::Updates(us) => Ok(us),
            _ => Ok(vec![]),
        }
    }

    async fn send_ping(&mut self) -> Result<(), InvocationError> {
        let req = tl::functions::Ping { ping_id: random_i64() };
        let wire = self.enc.pack(&req);
        send_frame(&mut self.stream, &wire, &self.frame_kind).await?;
        Ok(())
    }
}

// ─── Transport framing (multi-kind) ──────────────────────────────────────────

/// Send a framed message using the active transport kind.
async fn send_frame(
    stream: &mut TcpStream,
    data:   &[u8],
    kind:   &FrameKind,
) -> Result<(), InvocationError> {
    match kind {
        FrameKind::Abridged => send_abridged(stream, data).await,
        FrameKind::Intermediate => {
            stream.write_all(&(data.len() as u32).to_le_bytes()).await?;
            stream.write_all(data).await?;
            Ok(())
        }
        FrameKind::Full { .. } => {
            // seqno and CRC handled inside Connection; here we just prefix length
            // Full framing: [total_len 4B][seqno 4B][payload][crc32 4B]
            // But send_frame is called with already-encrypted payload.
            // We use a simplified approach: emit the same as Intermediate for now
            // and note that Full's seqno/CRC are transport-level, not app-level.
            stream.write_all(&(data.len() as u32).to_le_bytes()).await?;
            stream.write_all(data).await?;
            Ok(())
        }
    }
}

/// Receive a framed message.
async fn recv_frame(
    stream: &mut TcpStream,
    kind:   &mut FrameKind,
) -> Result<Vec<u8>, InvocationError> {
    match kind {
        FrameKind::Abridged => recv_abridged(stream).await,
        FrameKind::Intermediate | FrameKind::Full { .. } => {
            let mut len_buf = [0u8; 4];
            stream.read_exact(&mut len_buf).await?;
            let len = u32::from_le_bytes(len_buf) as usize;
            let mut buf = vec![0u8; len];
            stream.read_exact(&mut buf).await?;
            Ok(buf)
        }
    }
}

/// Send using Abridged framing (used for DH plaintext during connect).
async fn send_abridged(stream: &mut TcpStream, data: &[u8]) -> Result<(), InvocationError> {
    let words = data.len() / 4;
    if words < 0x7f {
        stream.write_all(&[words as u8]).await?;
    } else {
        let b = [0x7f, (words & 0xff) as u8, ((words >> 8) & 0xff) as u8, ((words >> 16) & 0xff) as u8];
        stream.write_all(&b).await?;
    }
    stream.write_all(data).await?;
    Ok(())
}

async fn recv_abridged(stream: &mut TcpStream) -> Result<Vec<u8>, InvocationError> {
    let mut h = [0u8; 1];
    stream.read_exact(&mut h).await?;
    let words = if h[0] < 0x7f {
        h[0] as usize
    } else {
        let mut b = [0u8; 3];
        stream.read_exact(&mut b).await?;
        let w = b[0] as usize | (b[1] as usize) << 8 | (b[2] as usize) << 16;
        // word count of 1 after 0xFF = Telegram 4-byte transport error code
        if w == 1 {
            let mut code_buf = [0u8; 4];
            stream.read_exact(&mut code_buf).await?;
            let code = i32::from_le_bytes(code_buf);
            return Err(InvocationError::Rpc(RpcError::from_telegram(code, "transport error")));
        }
        w
    };
    // Guard against implausibly large reads — a raw 4-byte transport error
    // whose first byte was mis-read as a word count causes a hang otherwise.
    if words == 0 || words > 0x8000 {
        return Err(InvocationError::Deserialize(
            format!("abridged: implausible word count {words} (possible transport error or framing mismatch)")
        ));
    }
    let mut buf = vec![0u8; words * 4];
    stream.read_exact(&mut buf).await?;
    Ok(buf)
}

/// Receive a plaintext (pre-auth) frame and deserialize it.
async fn recv_frame_plain<T: Deserializable>(
    stream: &mut TcpStream,
    _kind:  &FrameKind,
) -> Result<T, InvocationError> {
    let raw = recv_abridged(stream).await?; // DH always uses abridged for plaintext
    if raw.len() < 20 {
        return Err(InvocationError::Deserialize("plaintext frame too short".into()));
    }
    if u64::from_le_bytes(raw[..8].try_into().unwrap()) != 0 {
        return Err(InvocationError::Deserialize("expected auth_key_id=0 in plaintext".into()));
    }
    let body_len = u32::from_le_bytes(raw[16..20].try_into().unwrap()) as usize;
    let mut cur  = Cursor::from_slice(&raw[20..20 + body_len]);
    T::deserialize(&mut cur).map_err(Into::into)
}

// ─── MTProto envelope ─────────────────────────────────────────────────────────

enum EnvelopeResult {
    Payload(Vec<u8>),
    Updates(Vec<update::Update>),
    None,
}

fn unwrap_envelope(body: Vec<u8>) -> Result<EnvelopeResult, InvocationError> {
    if body.len() < 4 {
        return Err(InvocationError::Deserialize("body < 4 bytes".into()));
    }
    let cid = u32::from_le_bytes(body[..4].try_into().unwrap());

    match cid {
        ID_RPC_RESULT => {
            if body.len() < 12 {
                return Err(InvocationError::Deserialize("rpc_result too short".into()));
            }
            unwrap_envelope(body[12..].to_vec())
        }
        ID_RPC_ERROR => {
            if body.len() < 8 {
                return Err(InvocationError::Deserialize("rpc_error too short".into()));
            }
            let code    = i32::from_le_bytes(body[4..8].try_into().unwrap());
            let message = tl_read_string(&body[8..]).unwrap_or_default();
            Err(InvocationError::Rpc(RpcError::from_telegram(code, &message)))
        }
        ID_MSG_CONTAINER => {
            if body.len() < 8 {
                return Err(InvocationError::Deserialize("container too short".into()));
            }
            let count = u32::from_le_bytes(body[4..8].try_into().unwrap()) as usize;
            let mut pos = 8usize;
            let mut payload: Option<Vec<u8>> = None;
            let mut updates_buf: Vec<update::Update> = Vec::new();

            for _ in 0..count {
                if pos + 16 > body.len() { break; }
                let inner_len = u32::from_le_bytes(body[pos + 12..pos + 16].try_into().unwrap()) as usize;
                pos += 16;
                if pos + inner_len > body.len() { break; }
                let inner = body[pos..pos + inner_len].to_vec();
                pos += inner_len;
                match unwrap_envelope(inner)? {
                    EnvelopeResult::Payload(p)  => { payload = Some(p); }
                    EnvelopeResult::Updates(us) => { updates_buf.extend(us); }
                    EnvelopeResult::None        => {}
                }
            }
            if let Some(p) = payload {
                Ok(EnvelopeResult::Payload(p))
            } else if !updates_buf.is_empty() {
                Ok(EnvelopeResult::Updates(updates_buf))
            } else {
                Ok(EnvelopeResult::None)
            }
        }
        ID_GZIP_PACKED => {
            let bytes = tl_read_bytes(&body[4..]).unwrap_or_default();
            unwrap_envelope(gz_inflate(&bytes)?)
        }
        ID_PONG | ID_MSGS_ACK | ID_NEW_SESSION | ID_BAD_SERVER_SALT | ID_BAD_MSG_NOTIFY => {
            Ok(EnvelopeResult::None)
        }
        ID_UPDATES | ID_UPDATE_SHORT | ID_UPDATES_COMBINED
        | ID_UPDATE_SHORT_MSG | ID_UPDATE_SHORT_CHAT_MSG
        | ID_UPDATES_TOO_LONG => {
            Ok(EnvelopeResult::Updates(update::parse_updates(&body)))
        }
        _ => Ok(EnvelopeResult::Payload(body)),
    }
}

// ─── Utilities ────────────────────────────────────────────────────────────────

fn random_i64() -> i64 {
    let mut b = [0u8; 8];
    getrandom::getrandom(&mut b).expect("getrandom");
    i64::from_le_bytes(b)
}

fn tl_read_bytes(data: &[u8]) -> Option<Vec<u8>> {
    if data.is_empty() { return Some(vec![]); }
    let (len, start) = if data[0] < 254 { (data[0] as usize, 1) }
    else if data.len() >= 4 {
        (data[1] as usize | (data[2] as usize) << 8 | (data[3] as usize) << 16, 4)
    } else { return None; };
    if data.len() < start + len { return None; }
    Some(data[start..start + len].to_vec())
}

fn tl_read_string(data: &[u8]) -> Option<String> {
    tl_read_bytes(data).map(|b| String::from_utf8_lossy(&b).into_owned())
}

fn gz_inflate(data: &[u8]) -> Result<Vec<u8>, InvocationError> {
    use std::io::Read;
    let mut out = Vec::new();
    if flate2::read::GzDecoder::new(data).read_to_end(&mut out).is_ok() && !out.is_empty() {
        return Ok(out);
    }
    out.clear();
    flate2::read::ZlibDecoder::new(data)
        .read_to_end(&mut out)
        .map_err(|_| InvocationError::Deserialize("decompression failed".into()))?;
    Ok(out)
}
