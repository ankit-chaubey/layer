//! Multi-DC connection pool.
//!
//! Maintains one authenticated [`DcConnection`] per DC ID and routes RPC calls
//! to the correct DC automatically.  Auth keys are shared from the home DC via
//! `auth.exportAuthorization` / `auth.importAuthorization`.

use std::collections::HashMap;
use layer_tl_types as tl;
use layer_tl_types::{Cursor, Deserializable, RemoteCall};
use layer_mtproto::{EncryptedSession, Session, authentication as auth};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::{InvocationError, TransportKind, session::DcEntry};

// ─── DcConnection ─────────────────────────────────────────────────────────────

/// A single encrypted connection to one Telegram DC.
pub struct DcConnection {
    stream: TcpStream,
    enc:    EncryptedSession,
}

impl DcConnection {
    /// Connect and perform full DH handshake.
    pub async fn connect_raw(
        addr:      &str,
        socks5:    Option<&crate::socks5::Socks5Config>,
        transport: &TransportKind,
    ) -> Result<Self, InvocationError> {
        log::info!("[dc_pool] Connecting to {addr} …");
        let mut stream = Self::open_tcp(addr, socks5).await?;
        Self::send_transport_init(&mut stream, transport).await?;

        let mut plain = Session::new();

        let (req1, s1) = auth::step1().map_err(|e| InvocationError::Deserialize(e.to_string()))?;
        Self::send_plain_frame(&mut stream, &plain.pack(&req1).to_plaintext_bytes()).await?;
        let res_pq: tl::enums::ResPq = Self::recv_plain_frame(&mut stream).await?;

        let (req2, s2) = auth::step2(s1, res_pq).map_err(|e| InvocationError::Deserialize(e.to_string()))?;
        Self::send_plain_frame(&mut stream, &plain.pack(&req2).to_plaintext_bytes()).await?;
        let dh: tl::enums::ServerDhParams = Self::recv_plain_frame(&mut stream).await?;

        let (req3, s3) = auth::step3(s2, dh).map_err(|e| InvocationError::Deserialize(e.to_string()))?;
        Self::send_plain_frame(&mut stream, &plain.pack(&req3).to_plaintext_bytes()).await?;
        let ans: tl::enums::SetClientDhParamsAnswer = Self::recv_plain_frame(&mut stream).await?;

        let done = auth::finish(s3, ans).map_err(|e| InvocationError::Deserialize(e.to_string()))?;
        log::info!("[dc_pool] DH complete ✓ for {addr}");

        Ok(Self {
            stream,
            enc: EncryptedSession::new(done.auth_key, done.first_salt, done.time_offset),
        })
    }

    /// Connect with an already-known auth key (no DH needed).
    pub async fn connect_with_key(
        addr:        &str,
        auth_key:    [u8; 256],
        first_salt:  i64,
        time_offset: i32,
        socks5:      Option<&crate::socks5::Socks5Config>,
        transport:   &TransportKind,
    ) -> Result<Self, InvocationError> {
        let mut stream = Self::open_tcp(addr, socks5).await?;
        Self::send_transport_init(&mut stream, transport).await?;
        Ok(Self {
            stream,
            enc: EncryptedSession::new(auth_key, first_salt, time_offset),
        })
    }

    async fn open_tcp(
        addr:   &str,
        socks5: Option<&crate::socks5::Socks5Config>,
    ) -> Result<TcpStream, InvocationError> {
        match socks5 {
            Some(proxy) => proxy.connect(addr).await,
            None        => Ok(TcpStream::connect(addr).await?),
        }
    }

    async fn send_transport_init(
        stream:    &mut TcpStream,
        transport: &TransportKind,
    ) -> Result<(), InvocationError> {
        match transport {
            TransportKind::Abridged       => { stream.write_all(&[0xef]).await?; }
            TransportKind::Intermediate   => { stream.write_all(&[0xee, 0xee, 0xee, 0xee]).await?; }
            TransportKind::Full           => {} // no init byte
            TransportKind::Obfuscated { secret } => {
                let mut nonce = [0u8; 64];
                getrandom::getrandom(&mut nonce).map_err(|_| InvocationError::Deserialize("getrandom".into()))?;
                nonce[56] = 0xef; nonce[57] = 0xef; nonce[58] = 0xef; nonce[59] = 0xef;
                let (enc_key, enc_iv, _, _) = crate::transport_obfuscated::derive_keys(&nonce, secret.as_ref());
                let mut enc = crate::transport_obfuscated::ObfCipher::new(enc_key, enc_iv);
                let mut handshake = nonce;
                enc.apply(&mut handshake[56..]);
                stream.write_all(&handshake).await?;
            }
        }
        Ok(())
    }

    pub fn auth_key_bytes(&self) -> [u8; 256] { self.enc.auth_key_bytes() }
    pub fn first_salt(&self)     -> i64         { self.enc.salt }
    pub fn time_offset(&self)    -> i32         { self.enc.time_offset }

    pub async fn rpc_call<R: RemoteCall>(&mut self, req: &R) -> Result<Vec<u8>, InvocationError> {
        let wire = self.enc.pack(req);
        Self::send_abridged(&mut self.stream, &wire).await?;
        self.recv_rpc().await
    }

    async fn recv_rpc(&mut self) -> Result<Vec<u8>, InvocationError> {
        loop {
            let mut raw = Self::recv_abridged(&mut self.stream).await?;
            let msg = self.enc.unpack(&mut raw)
                .map_err(|e| InvocationError::Deserialize(e.to_string()))?;
            if msg.salt != 0 { self.enc.salt = msg.salt; }
            if msg.body.len() < 4 { return Ok(msg.body); }
            let cid = u32::from_le_bytes(msg.body[..4].try_into().unwrap());
            match cid {
                0xf35c6d01 /* rpc_result */ => {
                    if msg.body.len() >= 12 { return Ok(msg.body[12..].to_vec()); }
                    return Ok(msg.body);
                }
                0x2144ca19 /* rpc_error */ => {
                    if msg.body.len() < 8 {
                        return Err(InvocationError::Deserialize("rpc_error short".into()));
                    }
                    let code = i32::from_le_bytes(msg.body[4..8].try_into().unwrap());
                    let message = tl_read_string(&msg.body[8..]).unwrap_or_default();
                    return Err(InvocationError::Rpc(crate::RpcError::from_telegram(code, &message)));
                }
                0x347773c5 | 0x62d6b459 | 0x9ec20908 | 0xedab447b | 0xa7eff811 => continue,
                _ => return Ok(msg.body),
            }
        }
    }

    async fn send_abridged(stream: &mut TcpStream, data: &[u8]) -> Result<(), InvocationError> {
        let words = data.len() / 4;
        if words < 0x7f {
            stream.write_all(&[words as u8]).await?;
        } else {
            stream.write_all(&[0x7f, (words & 0xff) as u8, ((words >> 8) & 0xff) as u8, ((words >> 16) & 0xff) as u8]).await?;
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

    async fn send_plain_frame(stream: &mut TcpStream, data: &[u8]) -> Result<(), InvocationError> {
        Self::send_abridged(stream, data).await
    }

    async fn recv_plain_frame<T: Deserializable>(stream: &mut TcpStream) -> Result<T, InvocationError> {
        let raw = Self::recv_abridged(stream).await?;
        if raw.len() < 20 {
            return Err(InvocationError::Deserialize("plain frame too short".into()));
        }
        if u64::from_le_bytes(raw[..8].try_into().unwrap()) != 0 {
            return Err(InvocationError::Deserialize("expected auth_key_id=0 in plaintext".into()));
        }
        let body_len = u32::from_le_bytes(raw[16..20].try_into().unwrap()) as usize;
        let mut cur = Cursor::from_slice(&raw[20..20 + body_len]);
        T::deserialize(&mut cur).map_err(Into::into)
    }
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

// ─── DcPool ───────────────────────────────────────────────────────────────────

/// Pool of per-DC authenticated connections.
pub struct DcPool {
    conns:      HashMap<i32, DcConnection>,
    addrs:      HashMap<i32, String>,
    #[allow(dead_code)]
    home_dc_id: i32,
}

impl DcPool {
    pub fn new(home_dc_id: i32, dc_entries: &[DcEntry]) -> Self {
        let addrs = dc_entries.iter().map(|e| (e.dc_id, e.addr.clone())).collect();
        Self { conns: HashMap::new(), addrs, home_dc_id }
    }

    /// Returns true if a connection for `dc_id` already exists in the pool.
    pub fn has_connection(&self, dc_id: i32) -> bool {
        self.conns.contains_key(&dc_id)
    }

    /// Insert a pre-built connection into the pool.
    pub fn insert(&mut self, dc_id: i32, conn: DcConnection) {
        self.conns.insert(dc_id, conn);
    }

    /// Invoke a raw RPC call on the given DC.
    pub async fn invoke_on_dc<R: RemoteCall>(
        &mut self,
        dc_id:      i32,
        _dc_entries: &[DcEntry],
        req:        &R,
    ) -> Result<Vec<u8>, InvocationError> {
        let conn = self.conns.get_mut(&dc_id)
            .ok_or_else(|| InvocationError::Deserialize(format!("no connection for DC{dc_id}")))?;
        conn.rpc_call(req).await
    }

    /// Update the address table (called after `initConnection`).
    pub fn update_addrs(&mut self, entries: &[DcEntry]) {
        for e in entries { self.addrs.insert(e.dc_id, e.addr.clone()); }
    }

    /// Save the auth keys from pool connections back into the DC entry list.
    pub fn collect_keys(&self, entries: &mut Vec<DcEntry>) {
        for e in entries.iter_mut() {
            if let Some(conn) = self.conns.get(&e.dc_id) {
                e.auth_key    = Some(conn.auth_key_bytes());
                e.first_salt  = conn.first_salt();
                e.time_offset = conn.time_offset();
            }
        }
    }
}
