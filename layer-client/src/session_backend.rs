//! Pluggable session storage backend.
//!
//! # What changed vs the original
//!
//! | Before | After |
//! |---|---|
//! | Only `save(PersistedSession)` + `load()`: full round-trip for every change | New `update_dc`, `set_home_dc`, `update_state` allow granular writes |
//! | All methods sync (`io::Result`) | New methods are `async` (optional; default impls fall back to save/load) |
//! | No way to update a single DC key without touching everything else | `update_dc` only rewrites what changed |
//!
//! # Backward compatibility
//!
//! The existing `SessionBackend` trait is unchanged. The new methods have
//! default implementations that call `load` → mutate → `save`, so existing
//! backends (`BinaryFileBackend`, `InMemoryBackend`, `SqliteBackend`,
//! `LibSqlBackend`) continue to compile and work without modification.
//!
//! High-performance backends (e.g. a future Redis backend) can override the
//! granular methods to avoid the load/save round-trip.
//!
//! # Ported from
//!
//! ' `Session` trait (in `-session/src/session.rs`) exposes:
//! - `home_dc_id() -> i32`                           : cheap sync read
//! - `set_home_dc_id(dc_id) -> BoxFuture<'_, ()>`    : async write
//! - `dc_option(dc_id) -> Option<DcOption>`          : cheap sync read
//! - `set_dc_option(&DcOption) -> BoxFuture<'_, ()>` : async write
//! - `updates_state() -> BoxFuture<UpdatesState>`     : async read
//! - `set_update_state(UpdateState) -> BoxFuture<()>`: fine-grained async write
//!
//! We adopt the same pattern while keeping layer's `PersistedSession` struct.

use std::io;
use std::path::PathBuf;

use crate::session::{CachedPeer, DcEntry, PersistedSession, UpdatesStateSnap};

// Core trait (unchanged)

/// Synchronous snapshot backend: saves and loads the full session at once.
///
/// All built-in backends implement this. Higher-level code should prefer the
/// extension methods below (`update_dc`, `set_home_dc`, `update_state`) which
/// avoid unnecessary full-snapshot writes.
pub trait SessionBackend: Send + Sync {
    fn save(&self, session: &PersistedSession) -> io::Result<()>;
    fn load(&self) -> io::Result<Option<PersistedSession>>;
    fn delete(&self) -> io::Result<()>;

    /// Human-readable name for logging/debug output.
    fn name(&self) -> &str;

    // Granular helpers (default: load → mutate → save)
    //
    // These default implementations are correct but not optimal.
    // Backends that store data in a database (SQLite, libsql, Redis) should
    // override them to issue single-row UPDATE statements instead.

    /// Update a single DC entry without rewriting the entire session.
    ///
    /// Typically called after:
    /// - completing a DH handshake on a new DC (to persist its auth key)
    /// - receiving updated DC addresses from `help.getConfig`
    ///
    /// Ported from  `Session::set_dc_option`.
    fn update_dc(&self, entry: &DcEntry) -> io::Result<()> {
        let mut s = self.load()?.unwrap_or_default();
        // Replace existing entry or append
        if let Some(existing) = s.dcs.iter_mut().find(|d| d.dc_id == entry.dc_id) {
            *existing = entry.clone();
        } else {
            s.dcs.push(entry.clone());
        }
        self.save(&s)
    }

    /// Change the home DC without touching any other session data.
    ///
    /// Called after a successful `*_MIGRATE` redirect: the user's account
    /// now lives on a different DC.
    ///
    /// Ported from  `Session::set_home_dc_id`.
    fn set_home_dc(&self, dc_id: i32) -> io::Result<()> {
        let mut s = self.load()?.unwrap_or_default();
        s.home_dc_id = dc_id;
        self.save(&s)
    }

    /// Apply a single update-sequence change without a full save/load.
    ///
    /// Ported from  `Session::set_update_state(UpdateState)`.
    ///
    /// `update` is the new partial or full state to merge in.
    fn apply_update_state(&self, update: UpdateStateChange) -> io::Result<()> {
        let mut s = self.load()?.unwrap_or_default();
        update.apply_to(&mut s.updates_state);
        self.save(&s)
    }

    /// Cache a peer access hash without a full session save.
    ///
    /// This is lossy-on-default (full round-trip) but correct.
    /// Override in SQL backends to issue a single `INSERT OR REPLACE`.
    ///
    /// Ported from  `Session::cache_peer`.
    fn cache_peer(&self, peer: &CachedPeer) -> io::Result<()> {
        let mut s = self.load()?.unwrap_or_default();
        if let Some(existing) = s.peers.iter_mut().find(|p| p.id == peer.id) {
            *existing = peer.clone();
        } else {
            s.peers.push(peer.clone());
        }
        self.save(&s)
    }
}

// UpdateStateChange (mirrors  UpdateState enum)

/// A single update-sequence change, applied via [`SessionBackend::apply_update_state`].
///
///uses:
/// ```text
/// UpdateState::All(updates_state)
/// UpdateState::Primary { pts, date, seq }
/// UpdateState::Secondary { qts }
/// UpdateState::Channel { id, pts }
/// ```
///
/// We map this 1-to-1 to layer's `UpdatesStateSnap`.
#[derive(Debug, Clone)]
pub enum UpdateStateChange {
    /// Replace the entire state snapshot.
    All(UpdatesStateSnap),
    /// Update main sequence counters only (non-channel).
    Primary { pts: i32, date: i32, seq: i32 },
    /// Update the QTS counter (secret chats).
    Secondary { qts: i32 },
    /// Update the PTS for a specific channel.
    Channel { id: i64, pts: i32 },
}

impl UpdateStateChange {
    /// Apply `self` to `snap` in-place.
    pub fn apply_to(&self, snap: &mut UpdatesStateSnap) {
        match self {
            Self::All(new_snap) => *snap = new_snap.clone(),
            Self::Primary { pts, date, seq } => {
                snap.pts = *pts;
                snap.date = *date;
                snap.seq = *seq;
            }
            Self::Secondary { qts } => {
                snap.qts = *qts;
            }
            Self::Channel { id, pts } => {
                // Replace or insert per-channel pts
                if let Some(existing) = snap.channels.iter_mut().find(|c| c.0 == *id) {
                    existing.1 = *pts;
                } else {
                    snap.channels.push((*id, *pts));
                }
            }
        }
    }
}

// BinaryFileBackend

/// Stores the session in a compact binary file (v2 format).
pub struct BinaryFileBackend {
    path: PathBuf,
}

impl BinaryFileBackend {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
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
        match PersistedSession::load(&self.path) {
            Ok(s) => Ok(Some(s)),
            Err(e) => {
                let bak = self.path.with_extension("bak");
                tracing::warn!(
                    "[layer] Session file {:?} is corrupt ({e}); \
                     renaming to {:?} and starting fresh",
                    self.path,
                    bak
                );
                let _ = std::fs::rename(&self.path, &bak);
                Ok(None)
            }
        }
    }

    fn delete(&self) -> io::Result<()> {
        if self.path.exists() {
            std::fs::remove_file(&self.path)?;
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "binary-file"
    }

    // BinaryFileBackend: the default granular impls (load→mutate→save) are
    // fine since the format is a single compact binary blob. No override needed.
}

// InMemoryBackend

/// Ephemeral in-process session: nothing persisted to disk.
///
/// Override the granular methods to skip the clone overhead of the full
/// snapshot path (we're already in memory, so direct field mutations are
/// cheaper than clone→mutate→replace).
#[derive(Default)]
pub struct InMemoryBackend {
    data: std::sync::Mutex<Option<PersistedSession>>,
}

impl InMemoryBackend {
    pub fn new() -> Self {
        Self::default()
    }

    /// Test helper: get a snapshot of the current in-memory state.
    pub fn snapshot(&self) -> Option<PersistedSession> {
        self.data.lock().unwrap().clone()
    }
}

impl SessionBackend for InMemoryBackend {
    fn save(&self, s: &PersistedSession) -> io::Result<()> {
        *self.data.lock().unwrap() = Some(s.clone());
        Ok(())
    }

    fn load(&self) -> io::Result<Option<PersistedSession>> {
        Ok(self.data.lock().unwrap().clone())
    }

    fn delete(&self) -> io::Result<()> {
        *self.data.lock().unwrap() = None;
        Ok(())
    }

    fn name(&self) -> &str {
        "in-memory"
    }

    // Granular overrides: cheaper than load→clone→save

    fn update_dc(&self, entry: &DcEntry) -> io::Result<()> {
        let mut guard = self.data.lock().unwrap();
        let s = guard.get_or_insert_with(PersistedSession::default);
        if let Some(existing) = s.dcs.iter_mut().find(|d| d.dc_id == entry.dc_id) {
            *existing = entry.clone();
        } else {
            s.dcs.push(entry.clone());
        }
        Ok(())
    }

    fn set_home_dc(&self, dc_id: i32) -> io::Result<()> {
        let mut guard = self.data.lock().unwrap();
        let s = guard.get_or_insert_with(PersistedSession::default);
        s.home_dc_id = dc_id;
        Ok(())
    }

    fn apply_update_state(&self, update: UpdateStateChange) -> io::Result<()> {
        let mut guard = self.data.lock().unwrap();
        let s = guard.get_or_insert_with(PersistedSession::default);
        update.apply_to(&mut s.updates_state);
        Ok(())
    }

    fn cache_peer(&self, peer: &CachedPeer) -> io::Result<()> {
        let mut guard = self.data.lock().unwrap();
        let s = guard.get_or_insert_with(PersistedSession::default);
        if let Some(existing) = s.peers.iter_mut().find(|p| p.id == peer.id) {
            *existing = peer.clone();
        } else {
            s.peers.push(peer.clone());
        }
        Ok(())
    }
}

// StringSessionBackend

/// Portable base64 string session backend.
pub struct StringSessionBackend {
    data: std::sync::Mutex<String>,
}

impl StringSessionBackend {
    pub fn new(s: impl Into<String>) -> Self {
        Self {
            data: std::sync::Mutex::new(s.into()),
        }
    }

    pub fn current(&self) -> String {
        self.data.lock().unwrap().clone()
    }
}

impl SessionBackend for StringSessionBackend {
    fn save(&self, session: &PersistedSession) -> io::Result<()> {
        *self.data.lock().unwrap() = session.to_string();
        Ok(())
    }

    fn load(&self) -> io::Result<Option<PersistedSession>> {
        let s = self.data.lock().unwrap().clone();
        if s.trim().is_empty() {
            return Ok(None);
        }
        PersistedSession::from_string(&s).map(Some)
    }

    fn delete(&self) -> io::Result<()> {
        *self.data.lock().unwrap() = String::new();
        Ok(())
    }

    fn name(&self) -> &str {
        "string-session"
    }
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dc(id: i32) -> DcEntry {
        DcEntry {
            dc_id: id,
            addr: format!("1.2.3.{id}:443"),
            auth_key: None,
            first_salt: 0,
            time_offset: 0,
        }
    }

    fn make_peer(id: i64, hash: i64) -> CachedPeer {
        CachedPeer {
            id,
            access_hash: hash,
            is_channel: false,
        }
    }

    // InMemoryBackend: basic save/load

    #[test]
    fn inmemory_load_returns_none_when_empty() {
        let b = InMemoryBackend::new();
        assert!(b.load().unwrap().is_none());
    }

    #[test]
    fn inmemory_save_then_load_round_trips() {
        let b = InMemoryBackend::new();
        let mut s = PersistedSession::default();
        s.home_dc_id = 3;
        s.dcs.push(make_dc(3));
        b.save(&s).unwrap();

        let loaded = b.load().unwrap().unwrap();
        assert_eq!(loaded.home_dc_id, 3);
        assert_eq!(loaded.dcs.len(), 1);
    }

    #[test]
    fn inmemory_delete_clears_state() {
        let b = InMemoryBackend::new();
        let mut s = PersistedSession::default();
        s.home_dc_id = 2;
        b.save(&s).unwrap();
        b.delete().unwrap();
        assert!(b.load().unwrap().is_none());
    }

    // InMemoryBackend: granular methods

    #[test]
    fn inmemory_update_dc_inserts_new() {
        let b = InMemoryBackend::new();
        b.update_dc(&make_dc(4)).unwrap();
        let s = b.snapshot().unwrap();
        assert_eq!(s.dcs.len(), 1);
        assert_eq!(s.dcs[0].dc_id, 4);
    }

    #[test]
    fn inmemory_update_dc_replaces_existing() {
        let b = InMemoryBackend::new();
        b.update_dc(&make_dc(2)).unwrap();
        let mut updated = make_dc(2);
        updated.addr = "9.9.9.9:443".to_string();
        b.update_dc(&updated).unwrap();

        let s = b.snapshot().unwrap();
        assert_eq!(s.dcs.len(), 1);
        assert_eq!(s.dcs[0].addr, "9.9.9.9:443");
    }

    #[test]
    fn inmemory_set_home_dc() {
        let b = InMemoryBackend::new();
        b.set_home_dc(5).unwrap();
        assert_eq!(b.snapshot().unwrap().home_dc_id, 5);
    }

    #[test]
    fn inmemory_cache_peer_inserts() {
        let b = InMemoryBackend::new();
        b.cache_peer(&make_peer(100, 0xdeadbeef)).unwrap();
        let s = b.snapshot().unwrap();
        assert_eq!(s.peers.len(), 1);
        assert_eq!(s.peers[0].id, 100);
    }

    #[test]
    fn inmemory_cache_peer_updates_existing() {
        let b = InMemoryBackend::new();
        b.cache_peer(&make_peer(100, 111)).unwrap();
        b.cache_peer(&make_peer(100, 222)).unwrap();
        let s = b.snapshot().unwrap();
        assert_eq!(s.peers.len(), 1);
        assert_eq!(s.peers[0].access_hash, 222);
    }

    // UpdateStateChange

    #[test]
    fn update_state_primary() {
        let mut snap = UpdatesStateSnap {
            pts: 0,
            qts: 0,
            date: 0,
            seq: 0,
            channels: vec![],
        };
        UpdateStateChange::Primary {
            pts: 10,
            date: 20,
            seq: 30,
        }
        .apply_to(&mut snap);
        assert_eq!(snap.pts, 10);
        assert_eq!(snap.date, 20);
        assert_eq!(snap.seq, 30);
        assert_eq!(snap.qts, 0); // untouched
    }

    #[test]
    fn update_state_secondary() {
        let mut snap = UpdatesStateSnap {
            pts: 5,
            qts: 0,
            date: 0,
            seq: 0,
            channels: vec![],
        };
        UpdateStateChange::Secondary { qts: 99 }.apply_to(&mut snap);
        assert_eq!(snap.qts, 99);
        assert_eq!(snap.pts, 5); // untouched
    }

    #[test]
    fn update_state_channel_inserts() {
        let mut snap = UpdatesStateSnap {
            pts: 0,
            qts: 0,
            date: 0,
            seq: 0,
            channels: vec![],
        };
        UpdateStateChange::Channel { id: 12345, pts: 42 }.apply_to(&mut snap);
        assert_eq!(snap.channels, vec![(12345, 42)]);
    }

    #[test]
    fn update_state_channel_updates_existing() {
        let mut snap = UpdatesStateSnap {
            pts: 0,
            qts: 0,
            date: 0,
            seq: 0,
            channels: vec![(12345, 10), (67890, 5)],
        };
        UpdateStateChange::Channel { id: 12345, pts: 99 }.apply_to(&mut snap);
        // First channel updated, second untouched
        assert_eq!(snap.channels[0], (12345, 99));
        assert_eq!(snap.channels[1], (67890, 5));
    }

    #[test]
    fn apply_update_state_via_backend() {
        let b = InMemoryBackend::new();
        b.apply_update_state(UpdateStateChange::Primary {
            pts: 7,
            date: 8,
            seq: 9,
        })
        .unwrap();
        let s = b.snapshot().unwrap();
        assert_eq!(s.updates_state.pts, 7);
    }

    // Default impl (BinaryFileBackend trait shape via InMemory smoke)

    #[test]
    fn default_update_dc_via_trait_object() {
        let b: Box<dyn SessionBackend> = Box::new(InMemoryBackend::new());
        b.update_dc(&make_dc(1)).unwrap();
        b.update_dc(&make_dc(2)).unwrap();
        // Can't call snapshot() on trait object, but save/load must be consistent
        let loaded = b.load().unwrap().unwrap();
        assert_eq!(loaded.dcs.len(), 2);
    }
}
