//! Session persistence — saves auth key, salt, time offset, DC table,
//! update sequence counters (pts/qts/seq/date/per-channel pts), and
//! peer access-hash cache.
//!
//! ## Binary format versioning
//!
//! Every file starts with a single **version byte**:
//! - `0x01` — legacy format (DC table only, no update state or peers).
//! - `0x02` — current format (DC table + update state + peer cache).
//!
//! `load()` handles both.  `save()` always writes v2.

use std::collections::HashMap;
use std::io::{self, ErrorKind};
use std::path::Path;

// ─── DcEntry ──────────────────────────────────────────────────────────────────

/// One entry in the DC address table.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DcEntry {
    pub dc_id: i32,
    pub addr: String,
    pub auth_key: Option<[u8; 256]>,
    pub first_salt: i64,
    pub time_offset: i32,
}

// ─── UpdatesStateSnap ─────────────────────────────────────────────────────────

/// Snapshot of the MTProto update-sequence state that we persist so that
/// `catch_up: true` can call `updates.getDifference` with the *pre-shutdown* pts.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UpdatesStateSnap {
    /// Main persistence counter (messages, non-channel updates).
    pub pts: i32,
    /// Secondary counter for secret chats.
    pub qts: i32,
    /// Date of the last received update (Unix timestamp).
    pub date: i32,
    /// Combined-container sequence number.
    pub seq: i32,
    /// Per-channel persistence counters.  `(channel_id, pts)`.
    pub channels: Vec<(i64, i32)>,
}

impl UpdatesStateSnap {
    /// Returns `true` when we have a real state from the server (pts > 0).
    #[inline]
    pub fn is_initialised(&self) -> bool {
        self.pts > 0
    }

    /// Advance (or insert) a per-channel pts value.
    pub fn set_channel_pts(&mut self, channel_id: i64, pts: i32) {
        if let Some(entry) = self.channels.iter_mut().find(|c| c.0 == channel_id) {
            entry.1 = pts;
        } else {
            self.channels.push((channel_id, pts));
        }
    }

    /// Look up the stored pts for a channel, returns 0 if unknown.
    pub fn channel_pts(&self, channel_id: i64) -> i32 {
        self.channels
            .iter()
            .find(|c| c.0 == channel_id)
            .map(|c| c.1)
            .unwrap_or(0)
    }
}

// ─── CachedPeer ───────────────────────────────────────────────────────────────

/// A cached access-hash entry so that the peer can be addressed across restarts
/// without re-resolving it from Telegram.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CachedPeer {
    /// Bare Telegram peer ID (always positive).
    pub id: i64,
    /// Access hash bound to the current session.
    pub access_hash: i64,
    /// `true` → channel / supergroup.  `false` → user.
    pub is_channel: bool,
}

// ─── PersistedSession ─────────────────────────────────────────────────────────

/// Everything that needs to survive a process restart.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PersistedSession {
    pub home_dc_id: i32,
    pub dcs: Vec<DcEntry>,
    /// Update counters to enable reliable catch-up after a disconnect.
    pub updates_state: UpdatesStateSnap,
    /// Peer access-hash cache so that the client can reach out to any previously
    /// seen user or channel without re-resolving them.
    pub peers: Vec<CachedPeer>,
}

impl PersistedSession {
    // ── Serialise (v2) ────────────────────────────────────────────────────

    /// Encode the session to raw bytes (v2 binary format).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(512);

        b.push(0x02u8); // version

        b.extend_from_slice(&self.home_dc_id.to_le_bytes());

        b.push(self.dcs.len() as u8);
        for d in &self.dcs {
            b.extend_from_slice(&d.dc_id.to_le_bytes());
            match &d.auth_key {
                Some(k) => {
                    b.push(1);
                    b.extend_from_slice(k);
                }
                None => {
                    b.push(0);
                }
            }
            b.extend_from_slice(&d.first_salt.to_le_bytes());
            b.extend_from_slice(&d.time_offset.to_le_bytes());
            let ab = d.addr.as_bytes();
            b.push(ab.len() as u8);
            b.extend_from_slice(ab);
        }

        // update state
        b.extend_from_slice(&self.updates_state.pts.to_le_bytes());
        b.extend_from_slice(&self.updates_state.qts.to_le_bytes());
        b.extend_from_slice(&self.updates_state.date.to_le_bytes());
        b.extend_from_slice(&self.updates_state.seq.to_le_bytes());
        let ch = &self.updates_state.channels;
        b.extend_from_slice(&(ch.len() as u16).to_le_bytes());
        for &(cid, cpts) in ch {
            b.extend_from_slice(&cid.to_le_bytes());
            b.extend_from_slice(&cpts.to_le_bytes());
        }

        // peer cache
        b.extend_from_slice(&(self.peers.len() as u16).to_le_bytes());
        for p in &self.peers {
            b.extend_from_slice(&p.id.to_le_bytes());
            b.extend_from_slice(&p.access_hash.to_le_bytes());
            b.push(p.is_channel as u8);
        }

        b
    }

    /// Atomically save the session to `path`.
    ///
    /// Writes to `<path>.tmp` first, then renames into place so a crash
    /// mid-write never corrupts the existing session file.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, self.to_bytes())?;
        std::fs::rename(&tmp, path)
    }

    // ── Deserialise (v1 + v2) ─────────────────────────────────────────────

    /// Decode a session from raw bytes (v1 or v2 binary format).
    pub fn from_bytes(buf: &[u8]) -> io::Result<Self> {
        if buf.is_empty() {
            return Err(io::Error::new(ErrorKind::InvalidData, "empty session data"));
        }

        let mut p = 0usize;

        macro_rules! r {
            ($n:expr) => {{
                if p + $n > buf.len() {
                    return Err(io::Error::new(ErrorKind::InvalidData, "truncated session"));
                }
                let s = &buf[p..p + $n];
                p += $n;
                s
            }};
        }
        macro_rules! r_i32 {
            () => {
                i32::from_le_bytes(r!(4).try_into().unwrap())
            };
        }
        macro_rules! r_i64 {
            () => {
                i64::from_le_bytes(r!(8).try_into().unwrap())
            };
        }
        macro_rules! r_u8 {
            () => {
                r!(1)[0]
            };
        }
        macro_rules! r_u16 {
            () => {
                u16::from_le_bytes(r!(2).try_into().unwrap())
            };
        }

        let first_byte = r_u8!();

        let (home_dc_id, is_v2) = if first_byte == 0x02 {
            (r_i32!(), true)
        } else {
            let rest = r!(3);
            let mut bytes = [0u8; 4];
            bytes[0] = first_byte;
            bytes[1..4].copy_from_slice(rest);
            (i32::from_le_bytes(bytes), false)
        };

        let dc_count = r_u8!() as usize;
        let mut dcs = Vec::with_capacity(dc_count);
        for _ in 0..dc_count {
            let dc_id = r_i32!();
            let has_key = r_u8!();
            let auth_key = if has_key == 1 {
                let mut k = [0u8; 256];
                k.copy_from_slice(r!(256));
                Some(k)
            } else {
                None
            };
            let first_salt = r_i64!();
            let time_offset = r_i32!();
            let al = r_u8!() as usize;
            let addr = String::from_utf8_lossy(r!(al)).into_owned();
            dcs.push(DcEntry {
                dc_id,
                addr,
                auth_key,
                first_salt,
                time_offset,
            });
        }

        if !is_v2 {
            return Ok(Self {
                home_dc_id,
                dcs,
                updates_state: UpdatesStateSnap::default(),
                peers: Vec::new(),
            });
        }

        let pts = r_i32!();
        let qts = r_i32!();
        let date = r_i32!();
        let seq = r_i32!();
        let ch_count = r_u16!() as usize;
        let mut channels = Vec::with_capacity(ch_count);
        for _ in 0..ch_count {
            let cid = r_i64!();
            let cpts = r_i32!();
            channels.push((cid, cpts));
        }

        let peer_count = r_u16!() as usize;
        let mut peers = Vec::with_capacity(peer_count);
        for _ in 0..peer_count {
            let id = r_i64!();
            let access_hash = r_i64!();
            let is_channel = r_u8!() != 0;
            peers.push(CachedPeer {
                id,
                access_hash,
                is_channel,
            });
        }

        Ok(Self {
            home_dc_id,
            dcs,
            updates_state: UpdatesStateSnap {
                pts,
                qts,
                date,
                seq,
                channels,
            },
            peers,
        })
    }

    /// Decode a session from a URL-safe base64 string produced by [`to_string`].
    pub fn from_string(s: &str) -> io::Result<Self> {
        use base64::Engine as _;
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(s.trim())
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
        Self::from_bytes(&bytes)
    }

    pub fn load(path: &Path) -> io::Result<Self> {
        let buf = std::fs::read(path)?;
        Self::from_bytes(&buf)
    }
}

impl std::fmt::Display for PersistedSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use base64::Engine as _;
        f.write_str(&base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(self.to_bytes()))
    }
}

// ─── Bootstrap DC table ───────────────────────────────────────────────────────

/// Bootstrap DC address table (fallback if GetConfig fails).
pub fn default_dc_addresses() -> HashMap<i32, String> {
    [
        (1, "149.154.175.53:443"),
        (2, "149.154.167.51:443"),
        (3, "149.154.175.100:443"),
        (4, "149.154.167.91:443"),
        (5, "91.108.56.130:443"),
    ]
    .into_iter()
    .map(|(id, addr)| (id, addr.to_string()))
    .collect()
}
