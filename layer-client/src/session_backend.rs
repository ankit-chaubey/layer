//! Pluggable session storage backend.
//!
//! The [`SessionBackend`] trait abstracts over session persistence so that
//! callers can swap in an SQLite store, a custom binary file, an in-memory
//! store, or anything else.
//!
//! Two built-in backends are provided:
//! * [`BinaryFileBackend`] — the original binary file format (default).
//! * [`SqliteBackend`] — SQLite (requires the `sqlite-session` Cargo feature).

use std::io;
use std::path::PathBuf;
use crate::session::{DcEntry, PersistedSession};

// ─── Trait ────────────────────────────────────────────────────────────────────

/// An abstraction over where and how session data is persisted.
pub trait SessionBackend: Send + Sync {
    /// Persist the given session.
    fn save(&self, session: &PersistedSession) -> io::Result<()>;

    /// Load a previously persisted session, or return `None` if none exists.
    fn load(&self) -> io::Result<Option<PersistedSession>>;

    /// Remove the stored session (e.g. on sign-out).
    fn delete(&self) -> io::Result<()>;

    /// Human-readable name of this backend (for log messages).
    fn name(&self) -> &str;
}

// ─── BinaryFileBackend ────────────────────────────────────────────────────────

/// The default session backend — stores the session in a compact binary file.
///
/// This is the same format used by [`crate::session::PersistedSession`].
pub struct BinaryFileBackend {
    path: PathBuf,
}

impl BinaryFileBackend {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl SessionBackend for BinaryFileBackend {
    fn save(&self, session: &PersistedSession) -> io::Result<()> {
        session.save(&self.path)
    }

    fn load(&self) -> io::Result<Option<PersistedSession>> {
        if !self.path.exists() {
            return Ok(None);
        }
        PersistedSession::load(&self.path).map(Some)
    }

    fn delete(&self) -> io::Result<()> {
        if self.path.exists() {
            std::fs::remove_file(&self.path)?;
        }
        Ok(())
    }

    fn name(&self) -> &str { "binary-file" }
}

// ─── InMemoryBackend ─────────────────────────────────────────────────────────

/// An ephemeral session backend that stores nothing on disk.
///
/// Useful for testing or for bots that should always start fresh.
pub struct InMemoryBackend {
    data: std::sync::Mutex<Option<PersistedSessionData>>,
}

#[derive(Clone)]
struct PersistedSessionData {
    home_dc_id: i32,
    dcs:        Vec<DcEntry>,
}

impl InMemoryBackend {
    pub fn new() -> Self {
        Self { data: std::sync::Mutex::new(None) }
    }
}

impl Default for InMemoryBackend {
    fn default() -> Self { Self::new() }
}

impl SessionBackend for InMemoryBackend {
    fn save(&self, session: &PersistedSession) -> io::Result<()> {
        let mut lock = self.data.lock().unwrap();
        *lock = Some(PersistedSessionData {
            home_dc_id: session.home_dc_id,
            dcs:        session.dcs.clone(),
        });
        Ok(())
    }

    fn load(&self) -> io::Result<Option<PersistedSession>> {
        let lock = self.data.lock().unwrap();
        Ok(lock.as_ref().map(|d| PersistedSession {
            home_dc_id: d.home_dc_id,
            dcs:        d.dcs.clone(),
        }))
    }

    fn delete(&self) -> io::Result<()> {
        let mut lock = self.data.lock().unwrap();
        *lock = None;
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
    /// Creates two tables (`meta` and `dc_entries`) if they do not exist.
    ///
    /// Enable with the `sqlite-session` Cargo feature:
    /// ```toml
    /// [dependencies]
    /// layer-client = { version = "*", features = ["sqlite-session"] }
    /// ```
    pub struct SqliteBackend {
        path: PathBuf,
    }

    impl SqliteBackend {
        pub fn new(path: impl Into<PathBuf>) -> io::Result<Self> {
            let path = path.into();
            // Open and initialise the schema immediately so errors surface early.
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
                );",
            ).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            Ok(Self { path })
        }
    }

    impl SessionBackend for SqliteBackend {
        fn save(&self, session: &PersistedSession) -> io::Result<()> {
            let conn = Connection::open(&self.path)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            conn.execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES ('home_dc_id', ?1)",
                params![session.home_dc_id.to_string()],
            ).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            for dc in &session.dcs {
                let key_blob: Option<Vec<u8>> = dc.auth_key.map(|k| k.to_vec());
                conn.execute(
                    "INSERT OR REPLACE INTO dc_entries
                        (dc_id, addr, auth_key, first_salt, time_offset)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        dc.dc_id,
                        dc.addr,
                        key_blob,
                        dc.first_salt,
                        dc.time_offset,
                    ],
                ).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            }
            Ok(())
        }

        fn load(&self) -> io::Result<Option<PersistedSession>> {
            if !self.path.exists() {
                return Ok(None);
            }
            let conn = Connection::open(&self.path)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            let home_dc_id: Option<i32> = conn
                .query_row(
                    "SELECT value FROM meta WHERE key = 'home_dc_id'",
                    [],
                    |row| {
                        let v: String = row.get(0)?;
                        Ok(v.parse::<i32>().unwrap_or(2))
                    },
                )
                .ok();

            let home_dc_id = match home_dc_id {
                Some(id) => id,
                None => return Ok(None),
            };

            let mut stmt = conn
                .prepare("SELECT dc_id, addr, auth_key, first_salt, time_offset FROM dc_entries")
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            let dcs: Vec<DcEntry> = stmt
                .query_map([], |row| {
                    let dc_id:       i32         = row.get(0)?;
                    let addr:        String       = row.get(1)?;
                    let key_blob:    Option<Vec<u8>> = row.get(2)?;
                    let first_salt:  i64          = row.get(3)?;
                    let time_offset: i32          = row.get(4)?;
                    let auth_key = key_blob.and_then(|k| {
                        if k.len() == 256 {
                            let mut arr = [0u8; 256];
                            arr.copy_from_slice(&k);
                            Some(arr)
                        } else {
                            None
                        }
                    });
                    Ok(DcEntry { dc_id, addr, auth_key, first_salt, time_offset })
                })
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
                .filter_map(|r| r.ok())
                .collect();

            Ok(Some(PersistedSession { home_dc_id, dcs }))
        }

        fn delete(&self) -> io::Result<()> {
            if self.path.exists() {
                std::fs::remove_file(&self.path)?;
            }
            Ok(())
        }

        fn name(&self) -> &str { "sqlite" }
    }
}
