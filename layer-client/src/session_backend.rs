//! Pluggable session storage backend.
//!
//! Two built-in backends:
//! * [`BinaryFileBackend`] — compact binary file (default).
//! * [`SqliteBackend`]     — SQLite (`sqlite-session` feature).
//! * [`InMemoryBackend`]   — ephemeral, for tests / fresh-start bots.

use std::io;
use std::path::PathBuf;
use crate::session::{CachedPeer, DcEntry, PersistedSession, UpdatesStateSnap};

// ─── Trait ────────────────────────────────────────────────────────────────────

pub trait SessionBackend: Send + Sync {
    fn save(&self, session: &PersistedSession) -> io::Result<()>;
    fn load(&self) -> io::Result<Option<PersistedSession>>;
    fn delete(&self) -> io::Result<()>;
    fn name(&self) -> &str;
}

// ─── BinaryFileBackend ────────────────────────────────────────────────────────

/// Stores the session in a compact binary file (v2 format with update state + peer cache).
pub struct BinaryFileBackend {
    path: PathBuf,
}

impl BinaryFileBackend {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Returns the path this backend writes to.
    pub fn path(&self) -> &std::path::Path { &self.path }
}

impl SessionBackend for BinaryFileBackend {
    fn save(&self, session: &PersistedSession) -> io::Result<()> {
        session.save(&self.path)
    }
    fn load(&self) -> io::Result<Option<PersistedSession>> {
        if !self.path.exists() { return Ok(None); }
        PersistedSession::load(&self.path).map(Some)
    }
    fn delete(&self) -> io::Result<()> {
        if self.path.exists() { std::fs::remove_file(&self.path)?; }
        Ok(())
    }
    fn name(&self) -> &str { "binary-file" }
}

// ─── InMemoryBackend ─────────────────────────────────────────────────────────

/// Ephemeral session — nothing persisted to disk.
///
/// Useful for tests or bots that always start fresh. Note that access-hash
/// caches and update state are preserved across `save`/`load` calls *within
/// the same process*, which is what the reconnect path needs.
#[derive(Default)]
pub struct InMemoryBackend {
    data: std::sync::Mutex<Option<MemData>>,
}

#[derive(Clone)]
struct MemData {
    home_dc_id:    i32,
    dcs:           Vec<DcEntry>,
    updates_state: UpdatesStateSnap,
    peers:         Vec<CachedPeer>,
}

impl InMemoryBackend {
    pub fn new() -> Self { Self::default() }
}

impl SessionBackend for InMemoryBackend {
    fn save(&self, s: &PersistedSession) -> io::Result<()> {
        *self.data.lock().unwrap() = Some(MemData {
            home_dc_id:    s.home_dc_id,
            dcs:           s.dcs.clone(),
            updates_state: s.updates_state.clone(),
            peers:         s.peers.clone(),
        });
        Ok(())
    }
    fn load(&self) -> io::Result<Option<PersistedSession>> {
        Ok(self.data.lock().unwrap().as_ref().map(|d| PersistedSession {
            home_dc_id:    d.home_dc_id,
            dcs:           d.dcs.clone(),
            updates_state: d.updates_state.clone(),
            peers:         d.peers.clone(),
        }))
    }
    fn delete(&self) -> io::Result<()> {
        *self.data.lock().unwrap() = None;
        Ok(())
    }
    fn name(&self) -> &str { "in-memory" }
}

// ─── SqliteBackend ────────────────────────────────────────────────────────────

#[cfg(feature = "sqlite-session")]
pub use sqlite_backend::SqliteBackend;

#[cfg(feature = "sqlite-session")]
mod sqlite_backend {
    use super::*;
    use rusqlite::{Connection, params};

    /// SQLite-backed session store.
    ///
    /// Schema (auto-created on first open):
    /// - `meta`           — key/value string pairs (home_dc_id, pts, qts, date, seq)
    /// - `dc_entries`     — one row per DC
    /// - `channel_pts`    — per-channel pts counters
    /// - `peers`          — cached access hashes
    ///
    /// Enable with the `sqlite-session` Cargo feature:
    /// ```toml
    /// layer-client = { version = "*", features = ["sqlite-session"] }
    /// ```
    pub struct SqliteBackend { path: PathBuf }

    impl SqliteBackend {
        pub fn new(path: impl Into<PathBuf>) -> io::Result<Self> {
            let path = path.into();
            let conn = Connection::open(&path)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS meta (
                    key   TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS dc_entries (
                    dc_id       INTEGER PRIMARY KEY,
                    addr        TEXT    NOT NULL,
                    auth_key    BLOB,
                    first_salt  INTEGER NOT NULL DEFAULT 0,
                    time_offset INTEGER NOT NULL DEFAULT 0
                );
                CREATE TABLE IF NOT EXISTS channel_pts (
                    channel_id  INTEGER PRIMARY KEY,
                    pts         INTEGER NOT NULL DEFAULT 0
                );
                CREATE TABLE IF NOT EXISTS peers (
                    id          INTEGER PRIMARY KEY,
                    access_hash INTEGER NOT NULL,
                    is_channel  INTEGER NOT NULL DEFAULT 0
                );",
            ).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            Ok(Self { path })
        }

        fn conn(&self) -> io::Result<Connection> {
            Connection::open(&self.path)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
        }
    }

    impl SessionBackend for SqliteBackend {
        fn save(&self, s: &PersistedSession) -> io::Result<()> {
            let conn = self.conn()?;
            let e = |e: rusqlite::Error| io::Error::new(io::ErrorKind::Other, e);

            // meta
            for (k, v) in [
                ("home_dc_id", s.home_dc_id.to_string()),
                ("pts",  s.updates_state.pts.to_string()),
                ("qts",  s.updates_state.qts.to_string()),
                ("date", s.updates_state.date.to_string()),
                ("seq",  s.updates_state.seq.to_string()),
            ] {
                conn.execute(
                    "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
                    params![k, v],
                ).map_err(e)?;
            }

            // dc_entries
            for dc in &s.dcs {
                conn.execute(
                    "INSERT OR REPLACE INTO dc_entries
                        (dc_id, addr, auth_key, first_salt, time_offset)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        dc.dc_id,
                        dc.addr,
                        dc.auth_key.map(|k| k.to_vec()),
                        dc.first_salt,
                        dc.time_offset,
                    ],
                ).map_err(e)?;
            }

            // channel_pts
            conn.execute_batch("DELETE FROM channel_pts").map_err(e)?;
            for &(cid, cpts) in &s.updates_state.channels {
                conn.execute(
                    "INSERT INTO channel_pts (channel_id, pts) VALUES (?1, ?2)",
                    params![cid, cpts],
                ).map_err(e)?;
            }

            // peers
            conn.execute_batch("DELETE FROM peers").map_err(e)?;
            for p in &s.peers {
                conn.execute(
                    "INSERT INTO peers (id, access_hash, is_channel) VALUES (?1, ?2, ?3)",
                    params![p.id, p.access_hash, p.is_channel as i32],
                ).map_err(e)?;
            }

            Ok(())
        }

        fn load(&self) -> io::Result<Option<PersistedSession>> {
            if !self.path.exists() { return Ok(None); }
            let conn = self.conn()?;
            let e = |err: rusqlite::Error| io::Error::new(io::ErrorKind::Other, err);

            macro_rules! meta_i32 {
                ($key:expr, $default:expr) => {
                    conn.query_row(
                        "SELECT value FROM meta WHERE key = ?1",
                        params![$key],
                        |row| row.get::<_, String>(0),
                    ).ok()
                    .and_then(|v| v.parse::<i32>().ok())
                    .unwrap_or($default)
                };
            }

            let home_dc_id = meta_i32!("home_dc_id", 0);
            if home_dc_id == 0 { return Ok(None); }

            // dc_entries
            let mut stmt = conn.prepare(
                "SELECT dc_id, addr, auth_key, first_salt, time_offset FROM dc_entries"
            ).map_err(e)?;
            let dcs: Vec<DcEntry> = stmt.query_map([], |row| {
                let key_blob: Option<Vec<u8>> = row.get(2)?;
                let auth_key = key_blob.and_then(|k| {
                    if k.len() == 256 {
                        let mut a = [0u8; 256]; a.copy_from_slice(&k); Some(a)
                    } else { None }
                });
                Ok(DcEntry {
                    dc_id:       row.get(0)?,
                    addr:        row.get(1)?,
                    auth_key,
                    first_salt:  row.get(3)?,
                    time_offset: row.get(4)?,
                })
            }).map_err(e)?.filter_map(|r| r.ok()).collect();

            // update state
            let pts  = meta_i32!("pts",  0);
            let qts  = meta_i32!("qts",  0);
            let date = meta_i32!("date", 0);
            let seq  = meta_i32!("seq",  0);

            let mut ch_stmt = conn.prepare(
                "SELECT channel_id, pts FROM channel_pts"
            ).map_err(e)?;
            let channels: Vec<(i64, i32)> = ch_stmt
                .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i32>(1)?)))
                .map_err(e)?.filter_map(|r| r.ok()).collect();

            // peers
            let mut peer_stmt = conn.prepare(
                "SELECT id, access_hash, is_channel FROM peers"
            ).map_err(e)?;
            let peers: Vec<CachedPeer> = peer_stmt
                .query_map([], |row| Ok(CachedPeer {
                    id:          row.get(0)?,
                    access_hash: row.get(1)?,
                    is_channel:  row.get::<_, i32>(2)? != 0,
                }))
                .map_err(e)?.filter_map(|r| r.ok()).collect();

            Ok(Some(PersistedSession {
                home_dc_id,
                dcs,
                updates_state: UpdatesStateSnap { pts, qts, date, seq, channels },
                peers,
            }))
        }

        fn delete(&self) -> io::Result<()> {
            if self.path.exists() { std::fs::remove_file(&self.path)?; }
            Ok(())
        }

        fn name(&self) -> &str { "sqlite" }
    }
}
