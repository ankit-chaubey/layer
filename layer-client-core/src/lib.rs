//! High-level Telegram client — design mirrors grammers exactly.
//!
//! # Full flow
//!
//! ```rust,no_run
//! use layer_client_core::{Client, SignInError};
//!
//! // Reuse session if it exists, otherwise fresh DH + initConnection(GetConfig)
//! let mut client = Client::load_or_connect("session.bin", API_ID, API_HASH)?;
//!
//! if !client.is_authorized()? {
//!     let token = client.request_login_code(PHONE)?;
//!     let code  = prompt("Enter code: ");
//!
//!     match client.sign_in(&token, &code) {
//!         Ok(name)                                  => println!("Welcome, {name}!"),
//!         Err(SignInError::PasswordRequired(token)) => {
//!             let pw = prompt("Enter 2FA password: ");
//!             client.check_password(token, pw.trim())?;
//!         }
//!         Err(SignInError::InvalidCode)             => eprintln!("Wrong code"),
//!         Err(SignInError::SignUpRequired)           => eprintln!("Sign up via official app first"),
//!         Err(SignInError::Other(e))                => return Err(e.into()),
//!     }
//!     client.save_session("session.bin")?;
//! }
//!
//! client.send_message("me", "Hello from layer!")?;
//! ```

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

use layer_mtproto::transport::{AbridgedTransport, Transport};
use layer_mtproto::{EncryptedSession, Session, authentication as auth};
use layer_tl_types::{Cursor, Deserializable, RemoteCall};

pub use error::Error;
pub use sign_in_error::{SignInError, PasswordToken};
pub use login::LoginToken;

// ─── DC bootstrap addresses ───────────────────────────────────────────────────

const DC_ADDRESSES: &[(i32, &str)] = &[
    (1, "149.154.175.53:443"),
    (2, "149.154.167.51:443"),
    (3, "149.154.175.100:443"),
    (4, "149.154.167.91:443"),
    (5, "91.108.56.130:443"),
];

// ─── MTProto constructor IDs ──────────────────────────────────────────────────

const ID_RPC_RESULT:      u32 = 0xf35c6d01;
const ID_RPC_ERROR:       u32 = 0x2144ca19;
const ID_MSG_CONTAINER:   u32 = 0x73f1f8dc;
const ID_GZIP_PACKED:     u32 = 0x3072cfa1;
const ID_PONG:            u32 = 0x347773c5;
const ID_MSGS_ACK:        u32 = 0x62d6b459;
const ID_BAD_SERVER_SALT: u32 = 0xedab447b;
const ID_NEW_SESSION:     u32 = 0x9ec20908;
const ID_BAD_MSG_NOTIFY:  u32 = 0xa7eff811;

// ─── Error ────────────────────────────────────────────────────────────────────

mod error {
    use std::io;

    #[derive(Debug)]
    pub enum Error {
        Io(io::Error),
        Auth(layer_mtproto::authentication::Error),
        Decrypt(layer_mtproto::encrypted::DecryptError),
        Tl(layer_tl_types::deserialize::Error),
        /// Telegram returned an RPC error (e.g. PHONE_CODE_INVALID, code 420 FLOOD_WAIT_X).
        Rpc { code: i32, message: String },
        Proto(&'static str),
    }

    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Io(e)                 => write!(f, "IO: {e}"),
                Self::Auth(e)               => write!(f, "DH: {e}"),
                Self::Decrypt(e)            => write!(f, "Decrypt: {e}"),
                Self::Tl(e)                 => write!(f, "TL: {e}"),
                Self::Rpc { code, message } => write!(f, "RPC {code}: {message}"),
                Self::Proto(s)              => write!(f, "Protocol: {s}"),
            }
        }
    }
    impl std::error::Error for Error {}

    impl From<io::Error>                              for Error { fn from(e: io::Error) -> Self { Self::Io(e) } }
    impl From<layer_mtproto::authentication::Error>   for Error { fn from(e: layer_mtproto::authentication::Error) -> Self { Self::Auth(e) } }
    impl From<layer_mtproto::encrypted::DecryptError> for Error { fn from(e: layer_mtproto::encrypted::DecryptError) -> Self { Self::Decrypt(e) } }
    impl From<layer_tl_types::deserialize::Error>     for Error { fn from(e: layer_tl_types::deserialize::Error) -> Self { Self::Tl(e) } }
}

// ─── SignInError — mirrors grammers exactly ───────────────────────────────────

mod sign_in_error {
    use super::Error;

    /// Holds the server's 2FA challenge. Pass to [`super::Client::check_password`].
    pub struct PasswordToken {
        pub(crate) password: layer_tl_types::types::account::Password,
    }

    impl PasswordToken {
        /// The password hint set by the user, if any.
        pub fn hint(&self) -> Option<&str> {
            self.password.hint.as_deref()
        }
    }

    /// Errors that can occur during [`super::Client::sign_in`].
    ///
    /// Mirrors `grammers_client::SignInError`.
    #[derive(Debug)]
    pub enum SignInError {
        /// New number — must sign up via official app first.
        SignUpRequired,
        /// 2FA is enabled; pass the token to [`super::Client::check_password`].
        PasswordRequired(PasswordToken),
        /// The code was wrong or expired.
        InvalidCode,
        /// Generic error.
        Other(Error),
    }

    impl std::fmt::Display for SignInError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::SignUpRequired          => write!(f, "sign up required — use official app"),
                Self::PasswordRequired(_)     => write!(f, "2FA password required"),
                Self::InvalidCode             => write!(f, "invalid or expired code"),
                Self::Other(e)               => write!(f, "{e}"),
            }
        }
    }
    impl std::error::Error for SignInError {}
    impl From<Error> for SignInError { fn from(e: Error) -> Self { Self::Other(e) } }

    impl std::fmt::Debug for PasswordToken {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "PasswordToken {{ hint: {:?} }}", self.hint())
        }
    }
}

// ─── LoginToken ───────────────────────────────────────────────────────────────

mod login {
    /// Opaque token from [`super::Client::request_login_code`].
    /// Pass to [`super::Client::sign_in`].
    pub struct LoginToken {
        pub(crate) phone:           String,
        pub(crate) phone_code_hash: String,
    }
}

// ─── 2FA (SRP) — ported from grammers-crypto/two_factor_auth.rs ──────────────

mod two_factor_auth {
    use hmac::Hmac;
    use num_bigint::{BigInt, Sign};
    use num_traits::ops::euclid::Euclid;
    use sha2::{Digest, Sha256, Sha512};

    fn sha256(parts: &[&[u8]]) -> [u8; 32] {
        let mut h = Sha256::new();
        for p in parts { h.update(p); }
        h.finalize().into()
    }

    fn sh(data: &[u8], salt: &[u8]) -> [u8; 32] {
        sha256(&[salt, data, salt])
    }

    fn ph1(password: &[u8], salt1: &[u8], salt2: &[u8]) -> [u8; 32] {
        sh(&sh(password, salt1), salt2)
    }

    fn ph2(password: &[u8], salt1: &[u8], salt2: &[u8]) -> [u8; 32] {
        let hash1 = ph1(password, salt1, salt2);
        let mut dk = [0u8; 64];
        pbkdf2::pbkdf2::<Hmac<Sha512>>(&hash1, salt1, 100_000, &mut dk).unwrap();
        sh(&dk, salt2)
    }

    fn pad256(data: &[u8]) -> [u8; 256] {
        let mut out = [0u8; 256];
        let start = 256usize.saturating_sub(data.len());
        out[start..].copy_from_slice(&data[data.len().saturating_sub(256)..]);
        out
    }

    fn xor32(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
        let mut out = [0u8; 32];
        for i in 0..32 { out[i] = a[i] ^ b[i]; }
        out
    }

    /// Compute SRP `(M1, g_a)` for Telegram 2FA.
    /// Ported exactly from grammers `calculate_2fa`.
    pub fn calculate_2fa(
        salt1:    &[u8],
        salt2:    &[u8],
        p:        &[u8],
        g:        i32,
        g_b:      &[u8],
        a:        &[u8],
        password: impl AsRef<[u8]>,
    ) -> ([u8; 32], [u8; 256]) {
        let big_p  = BigInt::from_bytes_be(Sign::Plus, p);
        let g_b    = pad256(g_b);
        let a      = pad256(a);
        let g_hash = pad256(&[g as u8]);

        let big_g_b = BigInt::from_bytes_be(Sign::Plus, &g_b);
        let big_g   = BigInt::from(g as u32);
        let big_a   = BigInt::from_bytes_be(Sign::Plus, &a);

        // k = H(p | pad(g))
        let k    = sha256(&[p, &g_hash]);
        let big_k = BigInt::from_bytes_be(Sign::Plus, &k);

        // g_a = g^a mod p
        let g_a = big_g.modpow(&big_a, &big_p);
        let g_a = pad256(&g_a.to_bytes_be().1);

        // u = H(g_a | g_b)
        let u     = sha256(&[&g_a, &g_b]);
        let big_u = BigInt::from_bytes_be(Sign::Plus, &u);

        // x = PH2(password, salt1, salt2)
        let x     = ph2(password.as_ref(), salt1, salt2);
        let big_x = BigInt::from_bytes_be(Sign::Plus, &x);

        // v = g^x mod p,  k_v = k*v mod p
        let big_v  = big_g.modpow(&big_x, &big_p);
        let big_kv = (big_k * big_v) % &big_p;

        // t = (g_b - k_v) mod p  (positive)
        let big_t  = (big_g_b - big_kv).rem_euclid(&big_p);

        // s_a = t^(a + u*x) mod p
        let exp    = big_a + big_u * big_x;
        let big_sa = big_t.modpow(&exp, &big_p);

        // k_a = H(s_a)
        let k_a = sha256(&[&pad256(&big_sa.to_bytes_be().1)]);

        // M1 = H(H(p)^H(g) | H(salt1) | H(salt2) | g_a | g_b | k_a)
        let h_p   = sha256(&[p]);
        let h_g   = sha256(&[&g_hash]);
        let p_xg  = xor32(&h_p, &h_g);
        let m1    = sha256(&[&p_xg, &sha256(&[salt1]), &sha256(&[salt2]), &g_a, &g_b, &k_a]);

        (m1, g_a)
    }
}

// ─── DC option ────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct DcOption {
    addr:     String,
    auth_key: Option<[u8; 256]>,
}

// ─── Session persistence ──────────────────────────────────────────────────────

struct PersistedSession { home_dc_id: i32, dcs: Vec<PersistedDc> }
struct PersistedDc { dc_id: i32, auth_key: Option<[u8;256]>, first_salt: i64, time_offset: i32, addr: String }

impl PersistedSession {
    fn save(&self, path: &Path) -> io::Result<()> {
        let mut b = Vec::new();
        b.extend_from_slice(&self.home_dc_id.to_le_bytes());
        b.push(self.dcs.len() as u8);
        for d in &self.dcs {
            b.extend_from_slice(&d.dc_id.to_le_bytes());
            if let Some(k) = &d.auth_key { b.push(1); b.extend_from_slice(k); } else { b.push(0); }
            b.extend_from_slice(&d.first_salt.to_le_bytes());
            b.extend_from_slice(&d.time_offset.to_le_bytes());
            let ab = d.addr.as_bytes(); b.push(ab.len() as u8); b.extend_from_slice(ab);
        }
        fs::write(path, b)
    }

    fn load(path: &Path) -> io::Result<Self> {
        let buf = fs::read(path)?;
        let mut p = 0usize;
        macro_rules! r { ($n:expr) => {{ if p+$n > buf.len() { return Err(io::Error::new(io::ErrorKind::InvalidData,"truncated")); } let s=&buf[p..p+$n]; p+=$n; s }}; }
        let home_dc_id = i32::from_le_bytes(r!(4).try_into().unwrap());
        let dc_count   = r!(1)[0] as usize;
        let mut dcs    = Vec::with_capacity(dc_count);
        for _ in 0..dc_count {
            let dc_id      = i32::from_le_bytes(r!(4).try_into().unwrap());
            let has_key    = r!(1)[0];
            let auth_key   = if has_key==1 { let mut k=[0u8;256]; k.copy_from_slice(r!(256)); Some(k) } else { None };
            let first_salt  = i64::from_le_bytes(r!(8).try_into().unwrap());
            let time_offset = i32::from_le_bytes(r!(4).try_into().unwrap());
            let al = r!(1)[0] as usize;
            let addr = String::from_utf8_lossy(r!(al)).into_owned();
            dcs.push(PersistedDc { dc_id, auth_key, first_salt, time_offset, addr });
        }
        Ok(Self { home_dc_id, dcs })
    }
}

// ─── TCP transport ────────────────────────────────────────────────────────────

struct Tcp(TcpStream);
impl Tcp {
    fn connect(addr: &str) -> io::Result<Self> {
        let s = TcpStream::connect(addr)?;
        s.set_read_timeout(Some(Duration::from_secs(90)))?;
        s.set_write_timeout(Some(Duration::from_secs(10)))?;
        Ok(Self(s))
    }
}
impl Transport for Tcp {
    type Error = io::Error;
    fn send(&mut self, data: &[u8]) -> io::Result<()> { self.0.write_all(data) }
    fn recv(&mut self) -> io::Result<Vec<u8>> {
        let mut f=[0u8;1]; self.0.read_exact(&mut f)?;
        let words = if f[0]<0x7f { f[0] as usize }
            else { let mut b=[0u8;3]; self.0.read_exact(&mut b)?; b[0] as usize|(b[1] as usize)<<8|(b[2] as usize)<<16 };
        let mut buf=vec![0u8;words*4]; self.0.read_exact(&mut buf)?; Ok(buf)
    }
}

// ─── Connection (single DC) ───────────────────────────────────────────────────

struct Connection { transport: AbridgedTransport<Tcp>, enc: EncryptedSession }

impl Connection {
    /// Fresh connection — runs DH key exchange.
    fn connect_raw(addr: &str) -> Result<Self, Error> {
        let tcp = Tcp::connect(addr)?;
        let mut tr = AbridgedTransport::new(tcp);
        let mut plain = Session::new();

        let (req1, s1) = auth::step1()?;
        tr.send_message(&plain.pack(&req1).to_plaintext_bytes())?;
        let res_pq: layer_tl_types::enums::ResPq = recv_plain(&mut tr)?;

        let (req2, s2) = auth::step2(s1, res_pq)?;
        tr.send_message(&plain.pack(&req2).to_plaintext_bytes())?;
        let dh: layer_tl_types::enums::ServerDhParams = recv_plain(&mut tr)?;

        let (req3, s3) = auth::step3(s2, dh)?;
        tr.send_message(&plain.pack(&req3).to_plaintext_bytes())?;
        let ans: layer_tl_types::enums::SetClientDhParamsAnswer = recv_plain(&mut tr)?;

        let done = auth::finish(s3, ans)?;
        Ok(Self { transport: tr, enc: EncryptedSession::new(done.auth_key, done.first_salt, done.time_offset) })
    }

    /// Reuse saved auth key — no DH needed.
    /// Mirrors grammers `connect_with_auth`.
    fn connect_with_key(addr: &str, auth_key: [u8;256], first_salt: i64, time_offset: i32) -> Result<Self, Error> {
        let tcp = Tcp::connect(addr)?;
        Ok(Self { transport: AbridgedTransport::new(tcp), enc: EncryptedSession::new(auth_key, first_salt, time_offset) })
    }

    fn auth_key_bytes(&self) -> [u8;256] { self.enc.auth_key_bytes() }
    fn first_salt(&self)     -> i64       { self.enc.salt }
    fn time_offset(&self)    -> i32       { self.enc.time_offset }

    fn rpc_call<R: RemoteCall>(&mut self, req: &R) -> Result<Vec<u8>, Error> {
        let wire = self.enc.pack(req);
        self.transport.send_message(&wire)?;
        self.recv_rpc()
    }

    fn recv_rpc(&mut self) -> Result<Vec<u8>, Error> {
        loop {
            let mut raw = match self.transport.recv_message() {
                Ok(r) => r,
                Err(e) if e.kind()==io::ErrorKind::WouldBlock || e.kind()==io::ErrorKind::TimedOut => continue,
                Err(e) if e.kind()==io::ErrorKind::UnexpectedEof
                       || e.kind()==io::ErrorKind::ConnectionReset
                       || e.kind()==io::ErrorKind::ConnectionAborted => {
                    return Err(Error::Proto("server closed the connection"));
                }
                Err(e) => return Err(e.into()),
            };
            let msg = self.enc.unpack(&mut raw)?;
            if msg.salt != 0 { self.enc.salt = msg.salt; }
            match unwrap_envelope(msg.body)? {
                Some(p) => return Ok(p),
                None    => continue,
            }
        }
    }
}

// ─── Client ───────────────────────────────────────────────────────────────────

pub struct Client {
    conn:       Connection,
    home_dc_id: i32,
    dc_options: HashMap<i32, DcOption>,
    api_id:     i32,
    api_hash:   String,
}

impl Client {
    // ── Constructors ─────────────────────────────────────────────────────────

    /// Connect fresh: DH + `invokeWithLayer(initConnection(GetConfig))`.
    ///
    /// `initConnection` wraps `GetConfig` exactly like grammers' `SenderPoolRunner::connect_sender`.
    /// The Config response populates our DC address table for future migrations.
    pub fn connect(dc_addr: &str, api_id: i32, api_hash: &str) -> Result<Self, Error> {
        eprintln!("[layer] Connecting to {dc_addr} …");
        let conn = Connection::connect_raw(dc_addr)?;
        eprintln!("[layer] DH complete ✓");
        let mut client = Self {
            conn, home_dc_id: 2,
            dc_options: bootstrap_dc_options(),
            api_id, api_hash: api_hash.to_string(),
        };
        client.init_and_get_config()?;
        Ok(client)
    }

    /// Load saved session or connect fresh.
    ///
    /// On a saved session, reuses the auth key — mirrors grammers `connect_with_auth`.
    /// On no session file, defaults to DC2 (same as Telegram's recommended bootstrap DC).
    pub fn load_or_connect(session_path: impl AsRef<Path>, api_id: i32, api_hash: &str) -> Result<Self, Error> {
        let path = session_path.as_ref();
        if path.exists() {
            match PersistedSession::load(path) {
                Ok(s) => {
                    if let Some(dc) = s.dcs.iter().find(|d| d.dc_id == s.home_dc_id) {
                        if let Some(key) = dc.auth_key {
                            eprintln!("[layer] Loading session (DC{}) …", s.home_dc_id);
                            let conn = Connection::connect_with_key(&dc.addr, key, dc.first_salt, dc.time_offset)?;
                            let mut dc_options = bootstrap_dc_options();
                            for d in &s.dcs {
                                dc_options.insert(d.dc_id, DcOption { addr: d.addr.clone(), auth_key: d.auth_key });
                            }
                            let mut client = Self { conn, home_dc_id: s.home_dc_id, dc_options, api_id, api_hash: api_hash.to_string() };
                            client.init_and_get_config()?;
                            eprintln!("[layer] Session restored ✓");
                            return Ok(client);
                        }
                    }
                    eprintln!("[layer] Session incomplete — connecting fresh …");
                }
                Err(e) => eprintln!("[layer] Session load failed ({e}) — connecting fresh …"),
            }
        }
        Self::connect("149.154.167.51:443", api_id, api_hash)
    }

    // ── Session ───────────────────────────────────────────────────────────────

    /// Persist auth key + DC table. Call after successful sign-in.
    pub fn save_session(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        let dcs = self.dc_options.iter().map(|(&dc_id, opt)| PersistedDc {
            dc_id,
            auth_key:    opt.auth_key,
            first_salt:  if dc_id==self.home_dc_id { self.conn.first_salt()  } else { 0 },
            time_offset: if dc_id==self.home_dc_id { self.conn.time_offset() } else { 0 },
            addr:        opt.addr.clone(),
        }).collect();
        PersistedSession { home_dc_id: self.home_dc_id, dcs }.save(path.as_ref())?;
        eprintln!("[layer] Session saved ✓");
        Ok(())
    }

    // ── Auth ──────────────────────────────────────────────────────────────────

    /// Returns `true` if already logged in.
    /// Probes with `updates.getState` — same as grammers.
    pub fn is_authorized(&mut self) -> Result<bool, Error> {
        match self.conn.rpc_call(&layer_tl_types::functions::updates::GetState {}) {
            Ok(_)                             => Ok(true),
            Err(Error::Rpc { code: 401, .. }) => Ok(false),
            Err(e)                            => Err(e),
        }
    }

    /// Send login code. Handles `PHONE_MIGRATE_X` like grammers:
    /// disconnect, reconnect to correct DC, retry.
    pub fn request_login_code(&mut self, phone: &str) -> Result<LoginToken, Error> {
        use layer_tl_types::enums::auth::SentCode;
        let req  = self.make_send_code_req(phone);
        let body = match self.conn.rpc_call(&req) {
            Ok(b) => b,
            Err(Error::Rpc { code: 303, message }) => {
                let dc = parse_migrate_dc(&message)
                    .ok_or_else(|| Error::Rpc { code: 303, message: message.clone() })?;
                eprintln!("[layer] PHONE_MIGRATE_{dc} — reconnecting …");
                self.migrate_to(dc)?;
                self.conn.rpc_call(&req)?
            }
            Err(e) => return Err(e),
        };
        let mut cur = Cursor::from_slice(&body);
        let (hash, kind) = match layer_tl_types::enums::auth::SentCode::deserialize(&mut cur)? {
            SentCode::SentCode(c)        => (c.phone_code_hash, sent_code_type_name(&c.r#type)),
            SentCode::Success(_)         => return Err(Error::Proto("unexpected SentCode::Success")),
            SentCode::PaymentRequired(_) => return Err(Error::Proto("payment required")),
        };
        eprintln!("[layer] Code sent via {kind}");
        Ok(LoginToken { phone: phone.to_string(), phone_code_hash: hash })
    }

    /// Complete sign-in with the received code.
    ///
    /// Returns the display name on success.
    /// Returns `Err(SignInError::PasswordRequired(token))` if 2FA is enabled —
    /// pass the token to [`check_password`].
    ///
    /// Handles `USER_MIGRATE_X` exactly like grammers.
    pub fn sign_in(&mut self, token: &LoginToken, code: &str) -> Result<String, SignInError> {
        let req = layer_tl_types::functions::auth::SignIn {
            phone_number:       token.phone.clone(),
            phone_code_hash:    token.phone_code_hash.clone(),
            phone_code:         Some(code.trim().to_string()),
            email_verification: None,
        };

        let body = match self.conn.rpc_call(&req) {
            Ok(b) => b,
            // DC migration
            Err(Error::Rpc { code: 303, message }) => {
                let dc = parse_migrate_dc(&message)
                    .ok_or_else(|| Error::Rpc { code: 303, message: message.clone() })?;
                eprintln!("[layer] USER_MIGRATE_{dc} — reconnecting …");
                self.migrate_to(dc).map_err(SignInError::Other)?;
                self.conn.rpc_call(&req).map_err(SignInError::Other)?
            }
            // 2FA required — fetch password info and return PasswordRequired
            Err(Error::Rpc { message, .. }) if message.contains("SESSION_PASSWORD_NEEDED") => {
                let pw_token = self.get_password_info().map_err(SignInError::Other)?;
                return Err(SignInError::PasswordRequired(pw_token));
            }
            // Wrong/expired code
            Err(Error::Rpc { message, .. }) if message.starts_with("PHONE_CODE") => {
                return Err(SignInError::InvalidCode);
            }
            Err(e) => return Err(SignInError::Other(e)),
        };

        let mut cur = Cursor::from_slice(&body);
        match layer_tl_types::enums::auth::Authorization::deserialize(&mut cur).map_err(|e| SignInError::Other(e.into()))? {
            layer_tl_types::enums::auth::Authorization::Authorization(a) => {
                let name = extract_user_name(&a.user);
                eprintln!("[layer] Signed in ✓  Welcome, {name}!");
                Ok(name)
            }
            layer_tl_types::enums::auth::Authorization::SignUpRequired(_) =>
                Err(SignInError::SignUpRequired),
        }
    }

    /// Complete 2FA login with the user's password.
    ///
    /// `password_token` comes from `Err(SignInError::PasswordRequired(token))`.
    /// Mirrors grammers `check_password`.
    pub fn check_password(&mut self, password_token: PasswordToken, password: impl AsRef<[u8]>) -> Result<String, Error> {
        let pw   = password_token.password;
        let algo = pw.current_algo.ok_or(Error::Proto("no current_algo in Password"))?;

        let (salt1, salt2, p, g) = extract_password_params(&algo)?;

        let g_b        = pw.srp_b.ok_or(Error::Proto("no srp_b in Password"))?;
        let a          = pw.secure_random; // secure_random is always present (not optional)
        let srp_id     = pw.srp_id.ok_or(Error::Proto("no srp_id in Password"))?;

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

        let body    = self.conn.rpc_call(&req)?;
        let mut cur = Cursor::from_slice(&body);
        match layer_tl_types::enums::auth::Authorization::deserialize(&mut cur)? {
            layer_tl_types::enums::auth::Authorization::Authorization(a) => {
                let name = extract_user_name(&a.user);
                eprintln!("[layer] 2FA ✓  Welcome, {name}!");
                Ok(name)
            }
            layer_tl_types::enums::auth::Authorization::SignUpRequired(_) =>
                Err(Error::Proto("unexpected SignUpRequired after 2FA")),
        }
    }

    // ── Messaging ─────────────────────────────────────────────────────────────

    /// Send a text message to `peer`. Use `"me"` for Saved Messages.
    pub fn send_message(&mut self, peer: &str, text: &str) -> Result<(), Error> {
        let input_peer = match peer {
            "me" | "self" => layer_tl_types::enums::InputPeer::PeerSelf,
            _ => return Err(Error::Proto("only \"me\" supported — resolve peer first")),
        };
        let req = layer_tl_types::functions::messages::SendMessage {
            no_webpage: false, silent: false, background: false, clear_draft: false,
            noforwards: false, update_stickersets_order: false, invert_media: false,
            allow_paid_floodskip: false, peer: input_peer, reply_to: None,
            message: text.to_string(), random_id: random_i64(),
            reply_markup: None, entities: None, schedule_date: None,
            schedule_repeat_period: None, send_as: None, quick_reply_shortcut: None,
            effect: None, allow_paid_stars: None, suggested_post: None,
        };
        eprintln!("[layer] Sending message to {peer} …");
        self.conn.rpc_call(&req)?;
        eprintln!("[layer] Message sent ✓");
        Ok(())
    }

    // ── Raw invoke ────────────────────────────────────────────────────────────

    /// Invoke any TL function and return the deserialized response.
    pub fn invoke<R: RemoteCall>(&mut self, req: &R) -> Result<R::Return, Error> {
        let body = self.conn.rpc_call(req)?;
        let mut cur = Cursor::from_slice(&body);
        Ok(R::Return::deserialize(&mut cur)?)
    }

    // ── Private ───────────────────────────────────────────────────────────────

    /// `invokeWithLayer(initConnection(GetConfig {}))` — exactly like grammers.
    ///
    /// Wraps `GetConfig` so we receive the full DC list.
    /// Manually packs to avoid the `Deserializable` bound issue on generic wrappers.
    fn init_and_get_config(&mut self) -> Result<(), Error> {
        use layer_tl_types::functions::{InvokeWithLayer, InitConnection, help::GetConfig};
        let req = InvokeWithLayer {
            layer: layer_tl_types::LAYER,
            query: InitConnection {
                api_id: self.api_id,
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
        let wire = self.conn.enc.pack_serializable(&req);
        self.conn.transport.send_message(&wire)?;
        let body = self.conn.recv_rpc()?;
        let mut cur = Cursor::from_slice(&body);
        if let Ok(layer_tl_types::enums::Config::Config(cfg)) =
            layer_tl_types::enums::Config::deserialize(&mut cur)
        {
            self.update_dc_options(&cfg.dc_options);
            eprintln!("[layer] initConnection ✓  ({} DCs known)", self.dc_options.len());
        } else {
            eprintln!("[layer] initConnection ✓");
        }
        Ok(())
    }

    /// Parse DC options from Config and update table.
    /// Same filter as grammers `SenderPoolRunner::update_config`:
    /// skip media-only, CDN, tcpo-only, and IPv6.
    fn update_dc_options(&mut self, options: &[layer_tl_types::enums::DcOption]) {
        for opt in options {
            let layer_tl_types::enums::DcOption::DcOption(o) = opt;
            if o.media_only || o.cdn || o.tcpo_only || o.ipv6 { continue; }
            let addr = format!("{}:{}", o.ip_address, o.port);
            self.dc_options.entry(o.id)
                .or_insert_with(|| DcOption { addr: addr.clone(), auth_key: None })
                .addr = addr.clone();
        }
    }

    /// Disconnect from current DC and connect to `new_dc_id`.
    /// Mirrors grammers `disconnect_from_dc` + `set_home_dc_id` + `connect_sender`.
    fn migrate_to(&mut self, new_dc_id: i32) -> Result<(), Error> {
        let addr = self.dc_options.get(&new_dc_id)
            .map(|o| o.addr.clone())
            .or_else(|| DC_ADDRESSES.iter().find(|(id,_)| *id==new_dc_id).map(|(_,a)| a.to_string()))
            .unwrap_or_else(|| "149.154.167.51:443".to_string());

        eprintln!("[layer] Migrating to DC{new_dc_id} ({addr}) …");

        let saved_key = self.dc_options.get(&new_dc_id).and_then(|o| o.auth_key);
        let conn = if let Some(key) = saved_key {
            Connection::connect_with_key(&addr, key, 0, 0)?
        } else {
            Connection::connect_raw(&addr)?
        };

        // Save auth key for this DC
        let new_key = conn.auth_key_bytes();
        self.dc_options.entry(new_dc_id)
            .or_insert_with(|| DcOption { addr: addr.clone(), auth_key: None })
            .auth_key = Some(new_key);

        self.conn       = conn;
        self.home_dc_id = new_dc_id;
        self.init_and_get_config()?;
        eprintln!("[layer] Now on DC{new_dc_id} ✓");
        Ok(())
    }

    /// Fetch `account.getPassword` to get SRP challenge.
    /// Mirrors grammers `get_password_information`.
    fn get_password_info(&mut self) -> Result<PasswordToken, Error> {
        let body    = self.conn.rpc_call(&layer_tl_types::functions::account::GetPassword {})?;
        let mut cur = Cursor::from_slice(&body);
        let pw: layer_tl_types::types::account::Password =
            match layer_tl_types::enums::account::Password::deserialize(&mut cur)? {
                layer_tl_types::enums::account::Password::Password(p) => p,
            };
        Ok(PasswordToken { password: pw })
    }

    fn make_send_code_req(&self, phone: &str) -> layer_tl_types::functions::auth::SendCode {
        layer_tl_types::functions::auth::SendCode {
            phone_number: phone.to_string(),
            api_id:       self.api_id,
            api_hash:     self.api_hash.clone(),
            settings:     layer_tl_types::enums::CodeSettings::CodeSettings(
                layer_tl_types::types::CodeSettings {
                    allow_flashcall: false, current_number: false, allow_app_hash: false,
                    allow_missed_call: false, allow_firebase: false, unknown_number: false,
                    logout_tokens: None, token: None, app_sandbox: None,
                },
            ),
        }
    }
}

// ─── MTProto envelope unwrapper ───────────────────────────────────────────────

fn unwrap_envelope(body: Vec<u8>) -> Result<Option<Vec<u8>>, Error> {
    if body.len() < 4 { return Err(Error::Proto("body < 4 bytes")); }
    let cid = u32::from_le_bytes(body[..4].try_into().unwrap());
    match cid {
        ID_RPC_RESULT => {
            if body.len() < 12 { return Err(Error::Proto("rpc_result too short")); }
            unwrap_envelope(body[12..].to_vec())
        }
        ID_RPC_ERROR => {
            if body.len() < 8 { return Err(Error::Proto("rpc_error too short")); }
            let code    = i32::from_le_bytes(body[4..8].try_into().unwrap());
            let message = tl_read_string(&body[8..])?;
            Err(Error::Rpc { code, message })
        }
        ID_MSG_CONTAINER => {
            if body.len() < 8 { return Err(Error::Proto("container too short")); }
            let count   = u32::from_le_bytes(body[4..8].try_into().unwrap()) as usize;
            let mut pos = 8usize;
            let mut found: Option<Vec<u8>> = None;
            for _ in 0..count {
                if pos+16 > body.len() { break; }
                let inner_len = u32::from_le_bytes(body[pos+12..pos+16].try_into().unwrap()) as usize;
                pos += 16;
                if pos+inner_len > body.len() { break; }
                let inner = body[pos..pos+inner_len].to_vec();
                pos += inner_len;
                if let Some(p) = unwrap_envelope(inner)? { found = Some(p); }
            }
            Ok(found)
        }
        ID_GZIP_PACKED => {
            let bytes = tl_read_bytes(&body[4..])?;
            unwrap_envelope(gz_inflate(&bytes)?)
        }
        ID_PONG | ID_MSGS_ACK | ID_NEW_SESSION | ID_BAD_SERVER_SALT | ID_BAD_MSG_NOTIFY => Ok(None),
        _ => Ok(Some(body)),
    }
}

// ─── Plaintext helper (DH only) ───────────────────────────────────────────────

fn recv_plain<T: Deserializable>(tr: &mut AbridgedTransport<Tcp>) -> Result<T, Error> {
    let raw = tr.recv_message()?;
    if raw.len() < 20 { return Err(Error::Proto("plaintext frame too short")); }
    if u64::from_le_bytes(raw[..8].try_into().unwrap()) != 0 {
        return Err(Error::Proto("expected auth_key_id=0 in plaintext frame"));
    }
    let body_len = u32::from_le_bytes(raw[16..20].try_into().unwrap()) as usize;
    let mut cur  = Cursor::from_slice(&raw[20..20+body_len]);
    Ok(T::deserialize(&mut cur)?)
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn bootstrap_dc_options() -> HashMap<i32, DcOption> {
    DC_ADDRESSES.iter().map(|(id,addr)| (*id, DcOption { addr: addr.to_string(), auth_key: None })).collect()
}

/// Parse "PHONE_MIGRATE_5" → Some(5). Mirrors grammers `err.value`.
fn parse_migrate_dc(msg: &str) -> Option<i32> {
    msg.rsplit('_').next()?.parse().ok()
}

fn extract_password_params(algo: &layer_tl_types::enums::PasswordKdfAlgo)
    -> Result<(&[u8], &[u8], &[u8], i32), Error>
{
    match algo {
        layer_tl_types::enums::PasswordKdfAlgo::Sha256Sha256Pbkdf2Hmacsha512iter100000Sha256ModPow(a) => {
            Ok((&a.salt1, &a.salt2, &a.p, a.g))
        }
        _ => Err(Error::Proto("unsupported password KDF algorithm")),
    }
}

fn random_i64() -> i64 {
    let mut b = [0u8;8]; getrandom::getrandom(&mut b).expect("getrandom"); i64::from_le_bytes(b)
}

fn tl_read_bytes(data: &[u8]) -> Result<Vec<u8>, Error> {
    if data.is_empty() { return Ok(vec![]); }
    let (len, start) = if data[0]<254 { (data[0] as usize, 1) }
    else if data.len()>=4 { (data[1] as usize|(data[2] as usize)<<8|(data[3] as usize)<<16, 4) }
    else { return Err(Error::Proto("TL bytes header truncated")); };
    if data.len()<start+len { return Err(Error::Proto("TL bytes body truncated")); }
    Ok(data[start..start+len].to_vec())
}

fn tl_read_string(data: &[u8]) -> Result<String, Error> {
    tl_read_bytes(data).map(|b| String::from_utf8_lossy(&b).into_owned())
}

fn gz_inflate(data: &[u8]) -> Result<Vec<u8>, Error> {
    use std::io::Read;
    let mut out = Vec::new();
    if flate2::read::GzDecoder::new(data).read_to_end(&mut out).is_ok() && !out.is_empty() { return Ok(out); }
    out.clear();
    flate2::read::ZlibDecoder::new(data).read_to_end(&mut out).map_err(|_| Error::Proto("gzip failed"))?;
    Ok(out)
}

fn extract_user_name(user: &layer_tl_types::enums::User) -> String {
    match user {
        layer_tl_types::enums::User::User(u) =>
            format!("{} {}", u.first_name.as_deref().unwrap_or(""), u.last_name.as_deref().unwrap_or("")).trim().to_string(),
        layer_tl_types::enums::User::Empty(_) => "(unknown)".into(),
    }
}

fn sent_code_type_name(t: &layer_tl_types::enums::auth::SentCodeType) -> &'static str {
    use layer_tl_types::enums::auth::SentCodeType::*;
    match t {
        App(_) => "Telegram app", Sms(_) => "SMS", Call(_) => "phone call",
        FlashCall(_) => "flash call", MissedCall(_) => "missed call",
        FragmentSms(_) => "Fragment SMS", FirebaseSms(_) => "Firebase SMS",
        EmailCode(_) => "email code", SetUpEmailRequired(_) => "email setup required",
        SmsWord(_) => "SMS word", SmsPhrase(_) => "SMS phrase",
    }
}
