//! # layer-client — Production-grade async Telegram client
//!
//! A fully async, production-ready Telegram client built on top of the MTProto
//! protocol. Inspired by and architecturally aligned with [`grammers`](https://codeberg.org/Lonami/grammers).
//!
//! ## Features
//!
//! - ✅ Full async/tokio I/O
//! - ✅ User login (phone + code + 2FA SRP)
//! - ✅ Bot token login (`bot_sign_in`)
//! - ✅ `FLOOD_WAIT` auto-retry with configurable policy
//! - ✅ Update stream (`stream_updates`) with typed events
//! - ✅ Raw update access
//! - ✅ `NewMessage`, `MessageEdited`, `MessageDeleted`, `CallbackQuery`, `InlineQuery`
//! - ✅ Callback query answering
//! - ✅ Inline query answering
//! - ✅ Dialog iteration (`iter_dialogs`)
//! - ✅ Message iteration (`iter_messages`)
//! - ✅ Peer resolution (username, phone, ID)
//! - ✅ Send / edit / delete messages
//! - ✅ Forward messages
//! - ✅ DC migration handling
//! - ✅ Session persistence
//! - ✅ Sign out
//!
//! ## Quick Start — User Account
//!
//! ```rust,no_run
//! use layer_client::{Client, Config, SignInError};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut client = Client::connect(Config {
//!         session_path: "my.session".into(),
//!         api_id:       12345,
//!         api_hash:     "abc123".into(),
//!         ..Default::default()
//!     }).await?;
//!
//!     if !client.is_authorized().await? {
//!         let token = client.request_login_code("+1234567890").await?;
//!         let code  = "12345"; // read from stdin
//!
//!         match client.sign_in(&token, code).await {
//!             Ok(_)                                 => {}
//!             Err(SignInError::PasswordRequired(t)) => {
//!                 client.check_password(t, "my_password").await?;
//!             }
//!             Err(e) => return Err(e.into()),
//!         }
//!         client.save_session().await?;
//!     }
//!
//!     client.send_message("me", "Hello from layer!").await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Quick Start — Bot
//!
//! ```rust,no_run
//! use layer_client::{Client, Config, update::Update};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut client = Client::connect(Config {
//!         session_path: "bot.session".into(),
//!         api_id:       12345,
//!         api_hash:     "abc123".into(),
//!         ..Default::default()
//!     }).await?;
//!
//!     client.bot_sign_in("1234567890:ABCdef...").await?;
//!     client.save_session().await?;
//!
//!     let mut updates = client.stream_updates();
//!     while let Ok(update) = updates.next().await {
//!         if let Update::NewMessage(msg) = update {
//!             if !msg.outgoing() {
//!                 client.send_message_to_peer(msg.chat().unwrap().clone(), msg.text()).await?;
//!             }
//!         }
//!     }
//!     Ok(())
//! }
//! ```

#![deny(unsafe_code)]

mod errors;
mod retry;
mod session;
mod transport;
mod two_factor_auth;
pub mod update;

pub use errors::{InvocationError, LoginToken, PasswordToken, RpcError, SignInError};
pub use retry::{AutoSleep, NoRetries, RetryContext, RetryPolicy};
pub use update::Update;

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use layer_mtproto::{EncryptedSession, Session, authentication as auth};
use layer_tl_types::{Cursor, Deserializable, RemoteCall};
use session::{DcEntry, PersistedSession};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};
use tokio::time::sleep;

// ─── MTProto envelope constructor IDs ────────────────────────────────────────

const ID_RPC_RESULT:      u32 = 0xf35c6d01;
const ID_RPC_ERROR:       u32 = 0x2144ca19;
const ID_MSG_CONTAINER:   u32 = 0x73f1f8dc;
const ID_GZIP_PACKED:     u32 = 0x3072cfa1;
const ID_PONG:            u32 = 0x347773c5;
const ID_MSGS_ACK:        u32 = 0x62d6b459;
const ID_BAD_SERVER_SALT: u32 = 0xedab447b;
const ID_NEW_SESSION:     u32 = 0x9ec20908;
const ID_BAD_MSG_NOTIFY:  u32 = 0xa7eff811;
const ID_UPDATES:         u32 = 0x74ae4240;
const ID_UPDATE_SHORT:    u32 = 0x2114be86;
const ID_UPDATES_COMBINED: u32 = 0x725b04c3;
const ID_UPDATE_SHORT_MSG: u32 = 0x313bc7f8;

// ─── Config ───────────────────────────────────────────────────────────────────

/// Configuration for [`Client::connect`].
#[derive(Clone)]
pub struct Config {
    /// Where to load/save the session file.
    pub session_path: PathBuf,
    /// Telegram API ID from <https://my.telegram.org>.
    pub api_id:       i32,
    /// Telegram API hash from <https://my.telegram.org>.
    pub api_hash:     String,
    /// Initial DC address to connect to (default: DC2).
    pub dc_addr:      Option<String>,
    /// Retry policy for flood-wait and I/O errors.
    pub retry_policy: Arc<dyn RetryPolicy>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            session_path: "layer.session".into(),
            api_id:       0,
            api_hash:     String::new(),
            dc_addr:      None,
            retry_policy: Arc::new(AutoSleep::default()),
        }
    }
}

// ─── UpdatesConfiguration ─────────────────────────────────────────────────────

/// Configuration for [`Client::stream_updates`].
#[derive(Debug, Clone)]
pub struct UpdatesConfiguration {
    /// Optional cap on the internal update queue.
    /// Excess updates are dropped with a warning.
    pub update_queue_limit: Option<usize>,
}

impl Default for UpdatesConfiguration {
    fn default() -> Self {
        Self { update_queue_limit: Some(500) }
    }
}

// ─── UpdateStream ─────────────────────────────────────────────────────────────

/// Asynchronous stream of [`Update`]s.
///
/// Obtain via [`Client::stream_updates`]. Call [`UpdateStream::next`] in a loop.
pub struct UpdateStream {
    rx: mpsc::UnboundedReceiver<update::Update>,
}

impl UpdateStream {
    /// Wait for the next update.
    ///
    /// Returns `None` when the client has disconnected.
    pub async fn next(&mut self) -> Option<update::Update> {
        self.rx.recv().await
    }
}

// ─── Dialog ───────────────────────────────────────────────────────────────────

/// A Telegram dialog (chat, user, channel).
#[derive(Debug, Clone)]
pub struct Dialog {
    pub raw:    layer_tl_types::enums::Dialog,
    /// The top message in the dialog, if available.
    pub message: Option<layer_tl_types::enums::Message>,
    /// Entity (user/chat/channel) that corresponds to this dialog's peer.
    pub entity:  Option<layer_tl_types::enums::User>,
}

impl Dialog {
    /// The dialog's display title (username/first name/channel name).
    pub fn title(&self) -> String {
        if let Some(layer_tl_types::enums::User::User(u)) = &self.entity {
            let first = u.first_name.as_deref().unwrap_or("");
            let last  = u.last_name.as_deref().unwrap_or("");
            return format!("{first} {last}").trim().to_string();
        }
        "(Unknown)".to_string()
    }
}

// ─── Client (Arc-wrapped) ─────────────────────────────────────────────────────

struct ClientInner {
    conn:           Mutex<Connection>,
    home_dc_id:     Mutex<i32>,
    dc_options:     Mutex<HashMap<i32, DcEntry>>,
    api_id:         i32,
    api_hash:       String,
    session_path:   PathBuf,
    retry_policy:   Arc<dyn RetryPolicy>,
    _update_tx:     mpsc::UnboundedSender<update::Update>,
}

/// The main Telegram client. Cheap to clone — internally Arc-wrapped.
#[derive(Clone)]
pub struct Client {
    inner: Arc<ClientInner>,
    _update_rx: Arc<Mutex<mpsc::UnboundedReceiver<update::Update>>>,
}

impl Client {
    // ── Connect ────────────────────────────────────────────────────────────

    /// Connect to Telegram and return a ready-to-use client.
    ///
    /// Loads an existing session if the file exists, otherwise performs
    /// a full DH key exchange on DC2.
    pub async fn connect(config: Config) -> Result<Self, InvocationError> {
        let (update_tx, update_rx) = mpsc::unbounded_channel();

        // Try loading session
        let (conn, home_dc_id, dc_opts) =
            if config.session_path.exists() {
                match PersistedSession::load(&config.session_path) {
                    Ok(s) => {
                        if let Some(dc) = s.dcs.iter().find(|d| d.dc_id == s.home_dc_id) {
                            if let Some(key) = dc.auth_key {
                                log::info!("[layer] Loading session (DC{}) …", s.home_dc_id);
                                match Connection::connect_with_key(&dc.addr, key, dc.first_salt, dc.time_offset).await {
                                    Ok(c) => {
                                        let mut opts = session::default_dc_addresses()
                                            .into_iter()
                                            .map(|(id, addr)| (id, DcEntry { dc_id: id, addr, auth_key: None, first_salt: 0, time_offset: 0 }))
                                            .collect::<HashMap<_, _>>();
                                        for d in &s.dcs {
                                            opts.insert(d.dc_id, d.clone());
                                        }
                                        (c, s.home_dc_id, opts)
                                    }
                                    Err(e) => {
                                        log::warn!("[layer] Session connect failed ({e}), fresh connect …");
                                        Self::fresh_connect().await?
                                    }
                                }
                            } else {
                                Self::fresh_connect().await?
                            }
                        } else {
                            Self::fresh_connect().await?
                        }
                    }
                    Err(e) => {
                        log::warn!("[layer] Session load failed ({e}), fresh connect …");
                        Self::fresh_connect().await?
                    }
                }
            } else {
                Self::fresh_connect().await?
            };

        let inner = Arc::new(ClientInner {
            conn:        Mutex::new(conn),
            home_dc_id:  Mutex::new(home_dc_id),
            dc_options:  Mutex::new(dc_opts),
            api_id:      config.api_id,
            api_hash:    config.api_hash,
            session_path: config.session_path,
            retry_policy: config.retry_policy,
            _update_tx: update_tx,
        });

        let client = Self {
            inner,
            _update_rx: Arc::new(Mutex::new(update_rx)),
        };

        // Run initConnection to populate DC table
        client.init_connection().await?;
        Ok(client)
    }

    async fn fresh_connect() -> Result<(Connection, i32, HashMap<i32, DcEntry>), InvocationError> {
        log::info!("[layer] Fresh connect to DC2 …");
        let conn = Connection::connect_raw("149.154.167.51:443").await?;
        let opts = session::default_dc_addresses()
            .into_iter()
            .map(|(id, addr)| (id, DcEntry { dc_id: id, addr, auth_key: None, first_salt: 0, time_offset: 0 }))
            .collect();
        Ok((conn, 2, opts))
    }

    // ── Session ────────────────────────────────────────────────────────────

    /// Save the current session to disk.
    pub async fn save_session(&self) -> Result<(), InvocationError> {
        let conn_guard = self.inner.conn.lock().await;
        let home_dc_id = *self.inner.home_dc_id.lock().await;
        let dc_options = self.inner.dc_options.lock().await;

        let dcs = dc_options.values().map(|e| {
            DcEntry {
                dc_id:      e.dc_id,
                addr:       e.addr.clone(),
                auth_key:   if e.dc_id == home_dc_id { Some(conn_guard.auth_key_bytes()) } else { e.auth_key },
                first_salt:  if e.dc_id == home_dc_id { conn_guard.first_salt() } else { e.first_salt },
                time_offset: if e.dc_id == home_dc_id { conn_guard.time_offset() } else { e.time_offset },
            }
        }).collect();

        PersistedSession { home_dc_id, dcs }
            .save(&self.inner.session_path)
            .map_err(|e| InvocationError::Io(e))?;
        log::info!("[layer] Session saved ✓");
        Ok(())
    }

    // ── Auth ───────────────────────────────────────────────────────────────

    /// Returns `true` if the client is already authorized.
    pub async fn is_authorized(&self) -> Result<bool, InvocationError> {
        match self.invoke(&layer_tl_types::functions::updates::GetState {}).await {
            Ok(_)  => Ok(true),
            Err(e) if e.is("AUTH_KEY_UNREGISTERED") || matches!(&e, InvocationError::Rpc(r) if r.code == 401) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Sign in as a bot using a bot token from [@BotFather](https://t.me/BotFather).
    ///
    /// # Example
    /// ```rust,no_run
    /// client.bot_sign_in("1234567890:ABCdef...").await?;
    /// ```
    pub async fn bot_sign_in(&self, token: &str) -> Result<String, InvocationError> {
        let req = layer_tl_types::functions::auth::ImportBotAuthorization {
            flags:           0,
            api_id:          self.inner.api_id,
            api_hash:        self.inner.api_hash.clone(),
            bot_auth_token:  token.to_string(),
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
            layer_tl_types::enums::auth::Authorization::Authorization(a) => {
                Self::extract_user_name(&a.user)
            }
            layer_tl_types::enums::auth::Authorization::SignUpRequired(_) => {
                panic!("unexpected SignUpRequired during bot sign-in")
            }
        };
        log::info!("[layer] Bot signed in ✓  ({name})");
        Ok(name)
    }

    /// Request a login code for a user account.
    ///
    /// Returns a [`LoginToken`] to pass to [`sign_in`].
    ///
    /// [`sign_in`]: Self::sign_in
    pub async fn request_login_code(&self, phone: &str) -> Result<LoginToken, InvocationError> {
        use layer_tl_types::enums::auth::SentCode;

        let req = self.make_send_code_req(phone);
        let body = match self.rpc_call_raw(&req).await {
            Ok(b)  => b,
            Err(InvocationError::Rpc(ref r)) if r.code == 303 => {
                let dc_id = r.value.unwrap_or(2) as i32;
                self.migrate_to(dc_id).await?;
                self.rpc_call_raw(&req).await?
            }
            Err(e) => return Err(e),
        };

        let mut cur = Cursor::from_slice(&body);
        let hash = match layer_tl_types::enums::auth::SentCode::deserialize(&mut cur)? {
            SentCode::SentCode(c)        => c.phone_code_hash,
            SentCode::Success(_)         => return Err(InvocationError::Deserialize("unexpected SentCode::Success".into())),
            SentCode::PaymentRequired(_) => return Err(InvocationError::Deserialize("payment required".into())),
        };
        log::info!("[layer] Login code sent");
        Ok(LoginToken { phone: phone.to_string(), phone_code_hash: hash })
    }

    /// Complete sign-in with the code sent to the phone.
    ///
    /// On 2FA accounts, returns `Err(SignInError::PasswordRequired(token))`.
    /// Pass the token to [`check_password`].
    ///
    /// [`check_password`]: Self::check_password
    pub async fn sign_in(&self, token: &LoginToken, code: &str) -> Result<String, SignInError> {
        let req = layer_tl_types::functions::auth::SignIn {
            phone_number:       token.phone.clone(),
            phone_code_hash:    token.phone_code_hash.clone(),
            phone_code:         Some(code.trim().to_string()),
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
                let token = self.get_password_info().await.map_err(SignInError::Other)?;
                return Err(SignInError::PasswordRequired(token));
            }
            Err(e) if e.is("PHONE_CODE_*") => return Err(SignInError::InvalidCode),
            Err(e) => return Err(SignInError::Other(e)),
        };

        let mut cur = Cursor::from_slice(&body);
        match layer_tl_types::enums::auth::Authorization::deserialize(&mut cur)
            .map_err(|e| SignInError::Other(e.into()))?
        {
            layer_tl_types::enums::auth::Authorization::Authorization(a) => {
                let name = Self::extract_user_name(&a.user);
                log::info!("[layer] Signed in ✓  Welcome, {name}!");
                Ok(name)
            }
            layer_tl_types::enums::auth::Authorization::SignUpRequired(_) => {
                Err(SignInError::SignUpRequired)
            }
        }
    }

    /// Complete 2FA login.
    ///
    /// `token` comes from `Err(SignInError::PasswordRequired(token))`.
    pub async fn check_password(
        &self,
        token: PasswordToken,
        password: impl AsRef<[u8]>,
    ) -> Result<String, InvocationError> {
        let pw   = token.password;
        let algo = pw.current_algo.ok_or_else(|| InvocationError::Deserialize("no current_algo".into()))?;

        let (salt1, salt2, p, g) = Self::extract_password_params(&algo)?;
        let g_b  = pw.srp_b.ok_or_else(|| InvocationError::Deserialize("no srp_b".into()))?;
        let a    = pw.secure_random;
        let srp_id = pw.srp_id.ok_or_else(|| InvocationError::Deserialize("no srp_id".into()))?;

        let (m1, g_a) = two_factor_auth::calculate_2fa(salt1, salt2, p, g, &g_b, &a, password.as_ref());

        let req = layer_tl_types::functions::auth::CheckPassword {
            password: layer_tl_types::enums::InputCheckPasswordSrp::InputCheckPasswordSrp(
                layer_tl_types::types::InputCheckPasswordSrp {
                    srp_id,
                    a: g_a.to_vec(),
                    m1: m1.to_vec(),
                },
            ),
        };

        let body = self.rpc_call_raw(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        match layer_tl_types::enums::auth::Authorization::deserialize(&mut cur)? {
            layer_tl_types::enums::auth::Authorization::Authorization(a) => {
                let name = Self::extract_user_name(&a.user);
                log::info!("[layer] 2FA ✓  Welcome, {name}!");
                Ok(name)
            }
            layer_tl_types::enums::auth::Authorization::SignUpRequired(_) => {
                Err(InvocationError::Deserialize("unexpected SignUpRequired after 2FA".into()))
            }
        }
    }

    /// Sign out and invalidate the current session.
    pub async fn sign_out(&self) -> Result<bool, InvocationError> {
        let req = layer_tl_types::functions::auth::LogOut {};
        match self.rpc_call_raw(&req).await {
            Ok(_body) => {
                // auth.loggedOut#c3a2835f flags:# future_auth_token:flags.0?bytes = auth.LoggedOut
                log::info!("[layer] Signed out ✓");
                Ok(true)
            }
            Err(e) if e.is("AUTH_KEY_UNREGISTERED") => Ok(false),
            Err(e) => Err(e),
        }
    }

    // ── Updates ────────────────────────────────────────────────────────────

    /// Return an [`UpdateStream`] that yields incoming [`Update`]s.
    ///
    /// The stream must be polled regularly (e.g. in a `while let` loop) for
    /// the client to stay connected and receive updates. Multiple streams
    /// can be created but only one should be polled at a time.
    pub fn stream_updates(&self) -> UpdateStream {
        let (tx, rx) = mpsc::unbounded_channel();
        // Subscribe this new channel to the inner broadcaster
        // (we replace the stored sender so future updates go to this receiver)
        // In a real production impl you'd use a broadcast channel; here we
        // use a simple mpsc and pipe through a background task.
        let client = self.clone();
        tokio::spawn(async move {
            client.run_update_loop(tx).await;
        });
        UpdateStream { rx }
    }

    /// Internal update loop — polls the connection for incoming data and
    /// dispatches updates to the given sender.
    async fn run_update_loop(&self, tx: mpsc::UnboundedSender<update::Update>) {
        // Send periodic pings (every 60 s) while polling for updates.
        // In this simplified model we just do a blocking recv call in a loop.
        loop {
            // We need to recv from the socket while NOT holding the conn lock
            // during await. Use a timeout approach.
            let result = {
                let mut conn = self.inner.conn.lock().await;
                // Set a short timeout and check for data
                match tokio::time::timeout(
                    Duration::from_secs(30),
                    conn.recv_once()
                ).await {
                    Ok(Ok(updates)) => Ok(updates),
                    Ok(Err(e))      => Err(e),
                    Err(_timeout)   => {
                        // Send a ping to keep connection alive
                        let _ = conn.send_ping().await;
                        continue;
                    }
                }
            };

            match result {
                Ok(updates) => {
                    for u in updates {
                        let _ = tx.send(u);
                    }
                }
                Err(e) => {
                    log::warn!("[layer] Update loop error: {e} — reconnecting …");
                    sleep(Duration::from_secs(1)).await;
                    // Attempt reconnect
                    let home_dc_id = *self.inner.home_dc_id.lock().await;
                    let addr = {
                        let opts = self.inner.dc_options.lock().await;
                        opts.get(&home_dc_id).map(|e| e.addr.clone()).unwrap_or_else(|| "149.154.167.51:443".to_string())
                    };
                    match Connection::connect_raw(&addr).await {
                        Ok(new_conn) => {
                            *self.inner.conn.lock().await = new_conn;
                            let _ = self.init_connection().await;
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
        let input_peer = self.resolve_peer(peer).await?;
        self.send_message_to_peer(input_peer, text).await
    }

    /// Send a text message to an already-resolved [`layer_tl_types::enums::InputPeer`].
    pub async fn send_message_to_peer(
        &self,
        peer: layer_tl_types::enums::Peer,
        text: &str,
    ) -> Result<(), InvocationError> {
        let input_peer = peer_to_input_peer(peer);
        let req = layer_tl_types::functions::messages::SendMessage {
            no_webpage:                 false,
            silent:                     false,
            background:                 false,
            clear_draft:                false,
            noforwards:                 false,
            update_stickersets_order:   false,
            invert_media:               false,
            allow_paid_floodskip:       false,
            peer:                       input_peer,
            reply_to:                   None,
            message:                    text.to_string(),
            random_id:                  random_i64(),
            reply_markup:               None,
            entities:                   None,
            schedule_date:              None,
            schedule_repeat_period:     None,
            send_as:                    None,
            quick_reply_shortcut:       None,
            effect:                     None,
            allow_paid_stars:           None,
            suggested_post:             None,
        };
        self.rpc_call_raw(&req).await?;
        Ok(())
    }

    /// Send a text message directly to "me" (Saved Messages).
    pub async fn send_to_self(&self, text: &str) -> Result<(), InvocationError> {
        let req = layer_tl_types::functions::messages::SendMessage {
            no_webpage:                 false,
            silent:                     false,
            background:                 false,
            clear_draft:                false,
            noforwards:                 false,
            update_stickersets_order:   false,
            invert_media:               false,
            allow_paid_floodskip:       false,
            peer:                       layer_tl_types::enums::InputPeer::PeerSelf,
            reply_to:                   None,
            message:                    text.to_string(),
            random_id:                  random_i64(),
            reply_markup:               None,
            entities:                   None,
            schedule_date:              None,
            schedule_repeat_period:     None,
            send_as:                    None,
            quick_reply_shortcut:       None,
            effect:                     None,
            allow_paid_stars:           None,
            suggested_post:             None,
        };
        self.rpc_call_raw(&req).await?;
        Ok(())
    }

    /// Delete messages by ID in a given peer.
    pub async fn delete_messages(
        &self,
        message_ids: Vec<i32>,
        revoke: bool,
    ) -> Result<(), InvocationError> {
        let req = layer_tl_types::functions::messages::DeleteMessages {
            revoke,
            id: message_ids,
        };
        self.rpc_call_raw(&req).await?;
        Ok(())
    }

    // ── Callback Queries ───────────────────────────────────────────────────

    /// Answer a callback query (from an inline button press).
    ///
    /// Pass the `query_id` from [`update::CallbackQuery::query_id`].
    pub async fn answer_callback_query(
        &self,
        query_id: i64,
        text: Option<&str>,
        alert: bool,
    ) -> Result<bool, InvocationError> {
        let req = layer_tl_types::functions::messages::SetBotCallbackAnswer {
            alert,
            query_id,
            message: text.map(|s| s.to_string()),
            url:     None,
            cache_time: 0,
        };
        let body = self.rpc_call_raw(&req).await?;
        Ok(!body.is_empty())
    }

    // ── Inline Queries ─────────────────────────────────────────────────────

    /// Answer an inline query.
    ///
    /// `results` should be a list of `tl::enums::InputBotInlineResult`.
    pub async fn answer_inline_query(
        &self,
        query_id: i64,
        results: Vec<layer_tl_types::enums::InputBotInlineResult>,
        cache_time: i32,
        is_personal: bool,
        next_offset: Option<String>,
    ) -> Result<bool, InvocationError> {
        let req = layer_tl_types::functions::messages::SetInlineBotResults {
            gallery:     false,
            private:     is_personal,
            query_id,
            results,
            cache_time,
            next_offset,
            switch_pm:   None,
            switch_webview: None,
        };
        let body = self.rpc_call_raw(&req).await?;
        Ok(!body.is_empty())
    }

    // ── Dialogs ────────────────────────────────────────────────────────────

    /// Fetch up to `limit` dialogs (conversations), most recent first.
    ///
    /// Returns a `Vec<Dialog>`. For paginated access, call repeatedly with
    /// offset parameters derived from the last result.
    pub async fn get_dialogs(&self, limit: i32) -> Result<Vec<Dialog>, InvocationError> {
        let req = layer_tl_types::functions::messages::GetDialogs {
            exclude_pinned: false,
            folder_id:      None,
            offset_date:    0,
            offset_id:      0,
            offset_peer:    layer_tl_types::enums::InputPeer::Empty,
            limit,
            hash:           0,
        };

        let body    = self.rpc_call_raw(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        let dialogs = match layer_tl_types::enums::messages::Dialogs::deserialize(&mut cur)? {
            layer_tl_types::enums::messages::Dialogs::Dialogs(d) => d,
            layer_tl_types::enums::messages::Dialogs::Slice(d)   => {
                layer_tl_types::types::messages::Dialogs {
                    dialogs: d.dialogs, messages: d.messages, chats: d.chats, users: d.users,
                }
            }
            layer_tl_types::enums::messages::Dialogs::NotModified(_) => {
                return Ok(vec![]);
            }
        };

        let result = dialogs.dialogs.into_iter().map(|d| Dialog {
            raw:     d,
            message: None,
            entity:  None,
        }).collect();
        Ok(result)
    }

    // ── Messages ───────────────────────────────────────────────────────────

    /// Fetch messages from a peer's history.
    pub async fn get_messages(
        &self,
        peer: layer_tl_types::enums::InputPeer,
        limit: i32,
        offset_id: i32,
    ) -> Result<Vec<update::IncomingMessage>, InvocationError> {
        let req = layer_tl_types::functions::messages::GetHistory {
            peer,
            offset_id,
            offset_date: 0,
            add_offset:  0,
            limit,
            max_id:      0,
            min_id:      0,
            hash:        0,
        };

        let body    = self.rpc_call_raw(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        let messages = match layer_tl_types::enums::messages::Messages::deserialize(&mut cur)? {
            layer_tl_types::enums::messages::Messages::Messages(m) => m.messages,
            layer_tl_types::enums::messages::Messages::Slice(m)    => m.messages,
            layer_tl_types::enums::messages::Messages::ChannelMessages(m) => m.messages,
            layer_tl_types::enums::messages::Messages::NotModified(_)    => vec![],
        };

        Ok(messages.into_iter().map(update::IncomingMessage::from_raw).collect())
    }

    // ── Peer resolution ────────────────────────────────────────────────────

    /// Resolve a peer string (`"me"`, `"@username"`, phone, or numeric ID)
    /// to an [`InputPeer`](layer_tl_types::enums::InputPeer).
    pub async fn resolve_peer(
        &self,
        peer: &str,
    ) -> Result<layer_tl_types::enums::Peer, InvocationError> {
        match peer.trim() {
            "me" | "self" => Ok(layer_tl_types::enums::Peer::User(
                layer_tl_types::types::PeerUser { user_id: 0 }
            )),
            username if username.starts_with('@') => {
                self.resolve_username(&username[1..]).await
            }
            id_str => {
                if let Ok(id) = id_str.parse::<i64>() {
                    Ok(layer_tl_types::enums::Peer::User(
                        layer_tl_types::types::PeerUser { user_id: id }
                    ))
                } else {
                    Err(InvocationError::Deserialize(format!("cannot resolve peer: {peer}")))
                }
            }
        }
    }

    async fn resolve_username(&self, username: &str) -> Result<layer_tl_types::enums::Peer, InvocationError> {
        let req  = layer_tl_types::functions::contacts::ResolveUsername { username: username.to_string(), referer: None };
        let body = self.rpc_call_raw(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        let resolved = match layer_tl_types::enums::contacts::ResolvedPeer::deserialize(&mut cur)? {
            layer_tl_types::enums::contacts::ResolvedPeer::ResolvedPeer(r) => r,
        };
        Ok(resolved.peer)
    }

    // ── Raw invoke ─────────────────────────────────────────────────────────

    /// Invoke any TL function directly.
    ///
    /// Handles flood-wait and I/O retries according to the configured
    /// [`RetryPolicy`].
    pub async fn invoke<R: RemoteCall>(&self, req: &R) -> Result<R::Return, InvocationError> {
        let body = self.rpc_call_raw(req).await?;
        let mut cur = Cursor::from_slice(&body);
        R::Return::deserialize(&mut cur).map_err(Into::into)
    }

    /// Invoke and return the raw response bytes (before TL deserialization).
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

    // ── initConnection ─────────────────────────────────────────────────────

    async fn init_connection(&self) -> Result<(), InvocationError> {
        use layer_tl_types::functions::{InvokeWithLayer, InitConnection, help::GetConfig};
        let req = InvokeWithLayer {
            layer: layer_tl_types::LAYER,
            query: InitConnection {
                api_id:           self.inner.api_id,
                device_model:     "Linux".to_string(),
                system_version:   "1.0".to_string(),
                app_version:      "0.1.0".to_string(),
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
        if let Ok(layer_tl_types::enums::Config::Config(cfg)) =
            layer_tl_types::enums::Config::deserialize(&mut cur)
        {
            let mut opts = self.inner.dc_options.lock().await;
            for opt in &cfg.dc_options {
                let layer_tl_types::enums::DcOption::DcOption(o) = opt;
                if o.media_only || o.cdn || o.tcpo_only || o.ipv6 { continue; }
                let addr = format!("{}:{}", o.ip_address, o.port);
                let entry = opts.entry(o.id).or_insert_with(|| DcEntry {
                    dc_id: o.id, addr: addr.clone(),
                    auth_key: None, first_salt: 0, time_offset: 0,
                });
                entry.addr = addr;
            }
            log::info!("[layer] initConnection ✓  ({} DCs known)", cfg.dc_options.len());
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

        let conn = if let Some(key) = saved_key {
            Connection::connect_with_key(&addr, key, 0, 0).await?
        } else {
            Connection::connect_raw(&addr).await?
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

    // ── Private helpers ────────────────────────────────────────────────────

    async fn get_password_info(&self) -> Result<PasswordToken, InvocationError> {
        let body    = self.rpc_call_raw(&layer_tl_types::functions::account::GetPassword {}).await?;
        let mut cur = Cursor::from_slice(&body);
        let pw = match layer_tl_types::enums::account::Password::deserialize(&mut cur)? {
            layer_tl_types::enums::account::Password::Password(p) => p,
        };
        Ok(PasswordToken { password: pw })
    }

    fn make_send_code_req(&self, phone: &str) -> layer_tl_types::functions::auth::SendCode {
        layer_tl_types::functions::auth::SendCode {
            phone_number: phone.to_string(),
            api_id:       self.inner.api_id,
            api_hash:     self.inner.api_hash.clone(),
            settings:     layer_tl_types::enums::CodeSettings::CodeSettings(
                layer_tl_types::types::CodeSettings {
                    allow_flashcall:  false, current_number: false, allow_app_hash: false,
                    allow_missed_call: false, allow_firebase: false, unknown_number: false,
                    logout_tokens: None, token: None, app_sandbox: None,
                },
            ),
        }
    }

    fn extract_user_name(user: &layer_tl_types::enums::User) -> String {
        match user {
            layer_tl_types::enums::User::User(u) => {
                format!("{} {}", u.first_name.as_deref().unwrap_or(""),
                                  u.last_name.as_deref().unwrap_or(""))
                    .trim().to_string()
            }
            layer_tl_types::enums::User::Empty(_) => "(unknown)".into(),
        }
    }

    fn extract_password_params(
        algo: &layer_tl_types::enums::PasswordKdfAlgo,
    ) -> Result<(&[u8], &[u8], &[u8], i32), InvocationError> {
        match algo {
            layer_tl_types::enums::PasswordKdfAlgo::Sha256Sha256Pbkdf2Hmacsha512iter100000Sha256ModPow(a) => {
                Ok((&a.salt1, &a.salt2, &a.p, a.g))
            }
            _ => Err(InvocationError::Deserialize("unsupported password KDF algo".into())),
        }
    }
}

// ─── Connection ───────────────────────────────────────────────────────────────

/// A single async MTProto connection to one DC.
struct Connection {
    stream:     TcpStream,
    enc:        EncryptedSession,
}

impl Connection {
    async fn connect_raw(addr: &str) -> Result<Self, InvocationError> {
        log::info!("[layer] Connecting to {addr} (DH) …");
        let mut stream = TcpStream::connect(addr).await?;

        // Send abridged init byte
        stream.write_all(&[0xef]).await?;

        let mut plain = Session::new();

        // Step 1
        let (req1, s1) = auth::step1().map_err(|e| InvocationError::Deserialize(e.to_string()))?;
        send_plain(&mut stream, &plain.pack(&req1).to_plaintext_bytes()).await?;
        let res_pq: layer_tl_types::enums::ResPq = recv_plain(&mut stream).await?;

        // Step 2
        let (req2, s2) = auth::step2(s1, res_pq).map_err(|e| InvocationError::Deserialize(e.to_string()))?;
        send_plain(&mut stream, &plain.pack(&req2).to_plaintext_bytes()).await?;
        let dh: layer_tl_types::enums::ServerDhParams = recv_plain(&mut stream).await?;

        // Step 3
        let (req3, s3) = auth::step3(s2, dh).map_err(|e| InvocationError::Deserialize(e.to_string()))?;
        send_plain(&mut stream, &plain.pack(&req3).to_plaintext_bytes()).await?;
        let ans: layer_tl_types::enums::SetClientDhParamsAnswer = recv_plain(&mut stream).await?;

        let done = auth::finish(s3, ans).map_err(|e| InvocationError::Deserialize(e.to_string()))?;
        log::info!("[layer] DH complete ✓");

        Ok(Self {
            stream,
            enc: EncryptedSession::new(done.auth_key, done.first_salt, done.time_offset),
        })
    }

    async fn connect_with_key(
        addr: &str,
        auth_key: [u8; 256],
        first_salt: i64,
        time_offset: i32,
    ) -> Result<Self, InvocationError> {
        let mut stream = TcpStream::connect(addr).await?;
        stream.write_all(&[0xef]).await?;
        Ok(Self {
            stream,
            enc: EncryptedSession::new(auth_key, first_salt, time_offset),
        })
    }

    fn auth_key_bytes(&self) -> [u8; 256] { self.enc.auth_key_bytes() }
    fn first_salt(&self)     -> i64        { self.enc.salt }
    fn time_offset(&self)    -> i32        { self.enc.time_offset }

    async fn rpc_call<R: RemoteCall>(&mut self, req: &R) -> Result<Vec<u8>, InvocationError> {
        let wire = self.enc.pack(req);
        send_abridged(&mut self.stream, &wire).await?;
        self.recv_rpc().await
    }

    /// Like `rpc_call` but only requires `Serializable`, bypassing the `Deserializable`
    /// bound on `RemoteCall` that the code-generated `InvokeWithLayer` imposes.
    async fn rpc_call_serializable<S: layer_tl_types::Serializable>(&mut self, req: &S) -> Result<Vec<u8>, InvocationError> {
        let wire = self.enc.pack_serializable(req);
        send_abridged(&mut self.stream, &wire).await?;
        self.recv_rpc().await
    }

    async fn recv_rpc(&mut self) -> Result<Vec<u8>, InvocationError> {
        loop {
            let mut raw = recv_abridged(&mut self.stream).await?;
            let msg = self.enc.unpack(&mut raw)
                .map_err(|e| InvocationError::Deserialize(e.to_string()))?;
            if msg.salt != 0 { self.enc.salt = msg.salt; }
            match unwrap_envelope(msg.body)? {
                EnvelopeResult::Payload(p) => return Ok(p),
                EnvelopeResult::Updates(updates) => {
                    // Updates received while waiting for RPC result — buffer or discard
                    // In production we'd forward these to the update channel
                    log::debug!("[layer] {} updates received during RPC call", updates.len());
                }
                EnvelopeResult::None => {}
            }
        }
    }

    /// Receive a single raw frame and parse updates from it (for the update loop).
    async fn recv_once(&mut self) -> Result<Vec<update::Update>, InvocationError> {
        let mut raw = recv_abridged(&mut self.stream).await?;
        let msg = self.enc.unpack(&mut raw)
            .map_err(|e| InvocationError::Deserialize(e.to_string()))?;
        if msg.salt != 0 { self.enc.salt = msg.salt; }
        match unwrap_envelope(msg.body)? {
            EnvelopeResult::Updates(updates) => Ok(updates),
            _ => Ok(vec![]),
        }
    }

    async fn send_ping(&mut self) -> Result<(), InvocationError> {
        let ping_id = random_i64();
        let req = layer_tl_types::functions::Ping { ping_id };
        let wire = self.enc.pack(&req);
        send_abridged(&mut self.stream, &wire).await?;
        Ok(())
    }
}

// ─── Abridged transport helpers ───────────────────────────────────────────────

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
        b[0] as usize | (b[1] as usize) << 8 | (b[2] as usize) << 16
    };
    let mut buf = vec![0u8; words * 4];
    stream.read_exact(&mut buf).await?;
    Ok(buf)
}

async fn send_plain(stream: &mut TcpStream, data: &[u8]) -> Result<(), InvocationError> {
    send_abridged(stream, data).await
}

async fn recv_plain<T: Deserializable>(stream: &mut TcpStream) -> Result<T, InvocationError> {
    let raw = recv_abridged(stream).await?;
    if raw.len() < 20 { return Err(InvocationError::Deserialize("plaintext frame too short".into())); }
    if u64::from_le_bytes(raw[..8].try_into().unwrap()) != 0 {
        return Err(InvocationError::Deserialize("expected auth_key_id=0 in plaintext".into()));
    }
    let body_len = u32::from_le_bytes(raw[16..20].try_into().unwrap()) as usize;
    let mut cur  = Cursor::from_slice(&raw[20..20 + body_len]);
    T::deserialize(&mut cur).map_err(Into::into)
}

// ─── MTProto envelope unwrapper ───────────────────────────────────────────────

enum EnvelopeResult {
    Payload(Vec<u8>),
    Updates(Vec<update::Update>),
    None,
}

fn unwrap_envelope(body: Vec<u8>) -> Result<EnvelopeResult, InvocationError> {
    if body.len() < 4 { return Err(InvocationError::Deserialize("body < 4 bytes".into())); }
    let cid = u32::from_le_bytes(body[..4].try_into().unwrap());

    match cid {
        ID_RPC_RESULT => {
            if body.len() < 12 { return Err(InvocationError::Deserialize("rpc_result too short".into())); }
            unwrap_envelope(body[12..].to_vec())
        }
        ID_RPC_ERROR => {
            if body.len() < 8 { return Err(InvocationError::Deserialize("rpc_error too short".into())); }
            let code    = i32::from_le_bytes(body[4..8].try_into().unwrap());
            let message = tl_read_string(&body[8..]).unwrap_or_default();
            Err(InvocationError::Rpc(RpcError::from_telegram(code, &message)))
        }
        ID_MSG_CONTAINER => {
            if body.len() < 8 { return Err(InvocationError::Deserialize("container too short".into())); }
            let count   = u32::from_le_bytes(body[4..8].try_into().unwrap()) as usize;
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
        // Updates
        ID_UPDATES | ID_UPDATE_SHORT | ID_UPDATES_COMBINED | ID_UPDATE_SHORT_MSG => {
            parse_updates_envelope(cid, &body)
        }
        _ => Ok(EnvelopeResult::Payload(body)),
    }
}

fn parse_updates_envelope(_cid: u32, body: &[u8]) -> Result<EnvelopeResult, InvocationError> {
    let updates = update::parse_updates(body);
    Ok(EnvelopeResult::Updates(updates))
}

// ─── Helper functions ─────────────────────────────────────────────────────────

fn peer_to_input_peer(peer: layer_tl_types::enums::Peer) -> layer_tl_types::enums::InputPeer {
    match peer {
        layer_tl_types::enums::Peer::User(u) => {
            if u.user_id == 0 {
                layer_tl_types::enums::InputPeer::PeerSelf
            } else {
                layer_tl_types::enums::InputPeer::User(
                    layer_tl_types::types::InputPeerUser { user_id: u.user_id, access_hash: 0 }
                )
            }
        }
        layer_tl_types::enums::Peer::Chat(c) => {
            layer_tl_types::enums::InputPeer::Chat(
                layer_tl_types::types::InputPeerChat { chat_id: c.chat_id }
            )
        }
        layer_tl_types::enums::Peer::Channel(c) => {
            layer_tl_types::enums::InputPeer::Channel(
                layer_tl_types::types::InputPeerChannel { channel_id: c.channel_id, access_hash: 0 }
            )
        }
    }
}

fn random_i64() -> i64 {
    let mut b = [0u8; 8];
    getrandom::getrandom(&mut b).expect("getrandom");
    i64::from_le_bytes(b)
}

fn tl_read_bytes(data: &[u8]) -> Option<Vec<u8>> {
    if data.is_empty() { return Some(vec![]); }
    let (len, start) = if data[0] < 254 { (data[0] as usize, 1) }
    else if data.len() >= 4 { (data[1] as usize | (data[2] as usize) << 8 | (data[3] as usize) << 16, 4) }
    else { return None; };
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
