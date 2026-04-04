//! Update gap detection and recovery.
//!
//! Tracks `pts` / `qts` / `seq` / `date` plus **per-channel pts**, and
//! fills gaps via `updates.getDifference` (global) and
//! `updates.getChannelDifference` (per-channel, gap G-15).
//!
//! ## What "gap" means
//! Telegram guarantees updates arrive in order within a pts counter.
//! If `new_pts != local_pts + pts_count` there is a gap and we must
//! ask the server for the missed updates before processing this one.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use layer_tl_types as tl;
use layer_tl_types::{Cursor, Deserializable};

use crate::{Client, InvocationError, RpcError, attach_client_to_update, update};

// ─── PossibleGapBuffer (G-17) ─────────────────────────────────────────────────

/// How long to wait before declaring a pts jump a real gap (ms).
/// grammers uses a similar short window before triggering getDifference.
const POSSIBLE_GAP_DEADLINE_MS: u64 = 1_000;

// grammers: BOT_CHANNEL_DIFF_LIMIT = 100_000, USER_CHANNEL_DIFF_LIMIT = 100
/// Bots are allowed a much larger diff window (Telegram server-side limit).
const CHANNEL_DIFF_LIMIT_BOT: i32 = 100_000;
/// Regular users get a smaller window.
const CHANNEL_DIFF_LIMIT_USER: i32 = 100;

/// Buffers updates received during a possible-gap window so we don't fire
/// getDifference on every slightly out-of-order update.
#[derive(Default)]
pub struct PossibleGapBuffer {
    /// channel_id → (buffered_updates, window_start)
    channel: HashMap<i64, (Vec<update::Update>, Instant)>,
    /// Global buffered updates (non-channel pts gaps)
    global: Option<(Vec<update::Update>, Instant)>,
}

impl PossibleGapBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Buffer a global update during a possible-gap window.
    pub fn push_global(&mut self, upd: update::Update) {
        let entry = self
            .global
            .get_or_insert_with(|| (Vec::new(), Instant::now()));
        entry.0.push(upd);
    }

    /// Buffer a channel update during a possible-gap window.
    pub fn push_channel(&mut self, channel_id: i64, upd: update::Update) {
        let entry = self
            .channel
            .entry(channel_id)
            .or_insert_with(|| (Vec::new(), Instant::now()));
        entry.0.push(upd);
    }

    /// True if the global possible-gap deadline has elapsed.
    pub fn global_deadline_elapsed(&self) -> bool {
        self.global
            .as_ref()
            .map(|(_, t)| t.elapsed().as_millis() as u64 >= POSSIBLE_GAP_DEADLINE_MS)
            .unwrap_or(false)
    }

    /// True if a channel's possible-gap deadline has elapsed.
    pub fn channel_deadline_elapsed(&self, channel_id: i64) -> bool {
        self.channel
            .get(&channel_id)
            .map(|(_, t)| t.elapsed().as_millis() as u64 >= POSSIBLE_GAP_DEADLINE_MS)
            .unwrap_or(false)
    }

    /// True if the global buffer has any pending updates.
    pub fn has_global(&self) -> bool {
        self.global.is_some()
    }

    /// True if a channel buffer has pending updates.
    pub fn has_channel(&self, channel_id: i64) -> bool {
        self.channel.contains_key(&channel_id)
    }

    /// Drain global buffered updates.
    pub fn drain_global(&mut self) -> Vec<update::Update> {
        self.global.take().map(|(v, _)| v).unwrap_or_default()
    }

    /// Drain channel buffered updates.
    pub fn drain_channel(&mut self, channel_id: i64) -> Vec<update::Update> {
        self.channel
            .remove(&channel_id)
            .map(|(v, _)| v)
            .unwrap_or_default()
    }
}

// ─── PtsState ─────────────────────────────────────────────────────────────────

/// Full MTProto sequence-number state, including per-channel counters.
///
/// All fields are `pub` so that `connect()` can restore them from the
/// persisted session without going through an artificial constructor.
#[derive(Debug, Clone, Default)]
pub struct PtsState {
    /// Main pts counter (messages, non-channel updates).
    pub pts: i32,
    /// G-18: Secondary counter for secret-chat updates.
    pub qts: i32,
    /// Date of the last received update (Unix timestamp).
    pub date: i32,
    /// G-19: Combined-container sequence number.
    pub seq: i32,
    /// Per-channel pts counters.  `channel_id → pts`.
    pub channel_pts: HashMap<i64, i32>,
    /// G-16: Timestamp of last received update for deadline-based gap detection.
    pub last_update_at: Option<Instant>,
    /// Fix #4: Channels currently awaiting a getChannelDifference response.
    /// If a channel is in this set, no new gap-fill task is spawned for it —
    /// matches grammers' `getting_diff_for` guard that prevents 1 gap → N tasks.
    pub getting_diff_for: HashSet<i64>,
    /// Fix B2: Guard against concurrent global getDifference calls.
    /// Mirrors grammers' `getting_diff_for.contains(&Key::Common)`.
    /// Without this, two simultaneous gap detections both spawn get_difference(),
    /// which double-processes updates and corrupts pts state.
    pub getting_global_diff: bool,
}

impl PtsState {
    pub fn from_server_state(s: &tl::types::updates::State) -> Self {
        Self {
            pts: s.pts,
            qts: s.qts,
            date: s.date,
            seq: s.seq,
            channel_pts: HashMap::new(),
            last_update_at: Some(Instant::now()),
            getting_diff_for: HashSet::new(),
            getting_global_diff: false,
        }
    }

    /// Record that an update was received now (resets the deadline timer).
    pub fn touch(&mut self) {
        self.last_update_at = Some(Instant::now());
    }

    /// G-16: Returns true if no update has been received for > 15 minutes.
    pub fn deadline_exceeded(&self) -> bool {
        self.last_update_at
            .as_ref()
            .map(|t| t.elapsed().as_secs() > 15 * 60)
            .unwrap_or(false)
    }

    /// Check whether `new_pts` is in order given `pts_count` new updates.
    pub fn check_pts(&self, new_pts: i32, pts_count: i32) -> PtsCheckResult {
        let expected = self.pts + pts_count;
        if new_pts == expected {
            PtsCheckResult::Ok
        } else if new_pts > expected {
            PtsCheckResult::Gap {
                expected,
                got: new_pts,
            }
        } else {
            PtsCheckResult::Duplicate
        }
    }

    /// G-18: Check a qts value (secret chat updates).
    pub fn check_qts(&self, new_qts: i32, qts_count: i32) -> PtsCheckResult {
        let expected = self.qts + qts_count;
        if new_qts == expected {
            PtsCheckResult::Ok
        } else if new_qts > expected {
            PtsCheckResult::Gap {
                expected,
                got: new_qts,
            }
        } else {
            PtsCheckResult::Duplicate
        }
    }

    /// G-19: Check top-level seq for UpdatesCombined containers.
    pub fn check_seq(&self, _new_seq: i32, seq_start: i32) -> PtsCheckResult {
        if self.seq == 0 {
            return PtsCheckResult::Ok;
        } // uninitialised — accept
        let expected = self.seq + 1;
        if seq_start == expected {
            PtsCheckResult::Ok
        } else if seq_start > expected {
            PtsCheckResult::Gap {
                expected,
                got: seq_start,
            }
        } else {
            PtsCheckResult::Duplicate
        }
    }

    /// Check a per-channel pts value.
    pub fn check_channel_pts(
        &self,
        channel_id: i64,
        new_pts: i32,
        pts_count: i32,
    ) -> PtsCheckResult {
        let local = self.channel_pts.get(&channel_id).copied().unwrap_or(0);
        if local == 0 {
            return PtsCheckResult::Ok;
        }
        let expected = local + pts_count;
        if new_pts == expected {
            PtsCheckResult::Ok
        } else if new_pts > expected {
            PtsCheckResult::Gap {
                expected,
                got: new_pts,
            }
        } else {
            PtsCheckResult::Duplicate
        }
    }

    /// Advance the global pts.
    pub fn advance(&mut self, new_pts: i32) {
        if new_pts > self.pts {
            self.pts = new_pts;
        }
        self.touch();
    }

    /// Advance the qts (G-18).
    pub fn advance_qts(&mut self, new_qts: i32) {
        if new_qts > self.qts {
            self.qts = new_qts;
        }
        self.touch();
    }

    /// Advance seq (G-19).
    pub fn advance_seq(&mut self, new_seq: i32) {
        if new_seq > self.seq {
            self.seq = new_seq;
        }
    }

    /// Advance a per-channel pts.
    pub fn advance_channel(&mut self, channel_id: i64, new_pts: i32) {
        let entry = self.channel_pts.entry(channel_id).or_insert(0);
        if new_pts > *entry {
            *entry = new_pts;
        }
        self.touch();
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PtsCheckResult {
    Ok,
    Gap { expected: i32, got: i32 },
    Duplicate,
}

// ─── Client methods ───────────────────────────────────────────────────────────

impl Client {
    // ── Global getDifference ──────────────────────────────────────────────

    /// Fetch and replay any updates missed since the persisted pts.
    ///
    /// Fix B4: loops on `Difference::Slice` (partial response) until the server
    /// returns a final `Difference` or `Empty`, matching grammers' behaviour of
    /// never dropping a partial batch.  Previous code returned after one slice,
    /// silently losing all updates in subsequent slices.
    pub async fn get_difference(&self) -> Result<Vec<update::Update>, InvocationError> {
        // Fix B2: mark global diff in-flight so concurrent gap detections skip.
        // Cleared in every exit path below.
        self.inner.pts_state.lock().await.getting_global_diff = true;

        let result = self.get_difference_inner().await;

        // Always clear the guard, even on error.
        self.inner.pts_state.lock().await.getting_global_diff = false;

        result
    }

    async fn get_difference_inner(&self) -> Result<Vec<update::Update>, InvocationError> {
        use layer_tl_types::{Cursor, Deserializable};

        let mut all_updates: Vec<update::Update> = Vec::new();

        // Fix B4: loop until the server sends a final (non-Slice) response.
        loop {
            let (pts, qts, date) = {
                let s = self.inner.pts_state.lock().await;
                (s.pts, s.qts, s.date)
            };

            if pts == 0 {
                self.sync_pts_state().await?;
                return Ok(all_updates);
            }

            tracing::debug!("[layer] getDifference (pts={pts}, qts={qts}, date={date}) …");

            let req = tl::functions::updates::GetDifference {
                pts,
                pts_limit: None,
                pts_total_limit: None,
                date,
                qts,
                qts_limit: None,
            };

            let body = self.rpc_call_raw_pub(&req).await?;
            let mut cur = Cursor::from_slice(&body);
            let diff = tl::enums::updates::Difference::deserialize(&mut cur)?;

            match diff {
                tl::enums::updates::Difference::Empty(e) => {
                    let mut s = self.inner.pts_state.lock().await;
                    s.date = e.date;
                    s.seq = e.seq;
                    s.touch();
                    tracing::debug!("[layer] getDifference: empty (seq={})", e.seq);
                    return Ok(all_updates);
                }

                tl::enums::updates::Difference::Difference(d) => {
                    tracing::debug!(
                        "[layer] getDifference: {} messages, {} updates (final)",
                        d.new_messages.len(),
                        d.other_updates.len()
                    );
                    self.cache_users_slice_pub(&d.users).await;
                    self.cache_chats_slice_pub(&d.chats).await;
                    for msg in d.new_messages {
                        all_updates.push(update::Update::NewMessage(
                            update::IncomingMessage::from_raw(msg).with_client(self.clone()),
                        ));
                    }
                    for upd in d.other_updates {
                        all_updates.extend(update::from_single_update_pub(upd));
                    }
                    let tl::enums::updates::State::State(ns) = d.state;
                    let saved_channel_pts = {
                        let s = self.inner.pts_state.lock().await;
                        s.channel_pts.clone()
                    };
                    let mut new_state = PtsState::from_server_state(&ns);
                    // Preserve per-channel pts across the global reset.
                    for (cid, cpts) in saved_channel_pts {
                        new_state.channel_pts.entry(cid).or_insert(cpts);
                    }
                    // Preserve in-flight sets — we clear getting_global_diff ourselves.
                    new_state.getting_global_diff = true; // will be cleared by caller
                    {
                        let mut s = self.inner.pts_state.lock().await;
                        let getting_diff_for = std::mem::take(&mut s.getting_diff_for);
                        *s = new_state;
                        s.getting_diff_for = getting_diff_for;
                    }
                    // Final response — stop looping.
                    return Ok(all_updates);
                }

                tl::enums::updates::Difference::Slice(d) => {
                    // Fix B4: server has more data — apply intermediate_state and
                    // continue looping.  Old code returned here, losing all updates
                    // in subsequent slices.
                    tracing::debug!(
                        "[layer] getDifference slice: {} messages, {} updates — continuing",
                        d.new_messages.len(),
                        d.other_updates.len()
                    );
                    self.cache_users_slice_pub(&d.users).await;
                    self.cache_chats_slice_pub(&d.chats).await;
                    for msg in d.new_messages {
                        all_updates.push(update::Update::NewMessage(
                            update::IncomingMessage::from_raw(msg).with_client(self.clone()),
                        ));
                    }
                    for upd in d.other_updates {
                        all_updates.extend(update::from_single_update_pub(upd));
                    }
                    let tl::enums::updates::State::State(ns) = d.intermediate_state;
                    let saved_channel_pts = {
                        let s = self.inner.pts_state.lock().await;
                        s.channel_pts.clone()
                    };
                    let mut new_state = PtsState::from_server_state(&ns);
                    for (cid, cpts) in saved_channel_pts {
                        new_state.channel_pts.entry(cid).or_insert(cpts);
                    }
                    new_state.getting_global_diff = true;
                    {
                        let mut s = self.inner.pts_state.lock().await;
                        let getting_diff_for = std::mem::take(&mut s.getting_diff_for);
                        *s = new_state;
                        s.getting_diff_for = getting_diff_for;
                    }
                    // Loop: fetch the next slice.
                    continue;
                }

                tl::enums::updates::Difference::TooLong(d) => {
                    tracing::warn!(
                        "[layer] getDifference: TooLong (pts={}) — re-syncing",
                        d.pts
                    );
                    self.inner.pts_state.lock().await.pts = d.pts;
                    self.sync_pts_state().await?;
                    return Ok(all_updates);
                }
            }
        }
    }

    // ── G-15: Per-channel getChannelDifference ────────────────────────────

    /// Fetch missed updates for a single channel.
    pub async fn get_channel_difference(
        &self,
        channel_id: i64,
    ) -> Result<Vec<update::Update>, InvocationError> {
        let local_pts = self
            .inner
            .pts_state
            .lock()
            .await
            .channel_pts
            .get(&channel_id)
            .copied()
            .unwrap_or(0);

        let access_hash = self
            .inner
            .peer_cache
            .read()
            .await
            .channels
            .get(&channel_id)
            .copied()
            .unwrap_or(0);

        // No access hash in cache → we can't call getChannelDifference.
        // Attempting GetChannels with access_hash=0 also returns CHANNEL_INVALID,
        // so skip the call entirely and let the caller handle it.
        if access_hash == 0 {
            tracing::debug!(
                "[layer] channel {channel_id}: access_hash not cached, \
                 cannot call getChannelDifference — caller will remove from tracking"
            );
            return Err(InvocationError::Rpc(RpcError {
                code: 400,
                name: "CHANNEL_INVALID".into(),
                value: None,
            }));
        }

        tracing::debug!("[layer] getChannelDifference channel_id={channel_id} pts={local_pts}");

        let channel = tl::enums::InputChannel::InputChannel(tl::types::InputChannel {
            channel_id,
            access_hash,
        });

        // grammers: bots get BOT_CHANNEL_DIFF_LIMIT (100_000), users get USER_CHANNEL_DIFF_LIMIT (100)
        let diff_limit = if self.inner.is_bot.load(std::sync::atomic::Ordering::Relaxed) {
            CHANNEL_DIFF_LIMIT_BOT
        } else {
            CHANNEL_DIFF_LIMIT_USER
        };

        let req = tl::functions::updates::GetChannelDifference {
            force: false,
            channel,
            filter: tl::enums::ChannelMessagesFilter::Empty,
            pts: local_pts.max(1),
            limit: diff_limit,
        };

        let body = match self.rpc_call_raw_pub(&req).await {
            Ok(b) => b,
            Err(InvocationError::Rpc(ref e)) if e.name == "PERSISTENT_TIMESTAMP_OUTDATED" => {
                // G-20: treat as empty diff — retry next gap
                tracing::debug!("[layer] G-20 PERSISTENT_TIMESTAMP_OUTDATED — skipping diff");
                return Ok(vec![]);
            }
            Err(e) => return Err(e),
        };
        let mut cur = Cursor::from_slice(&body);
        let diff = tl::enums::updates::ChannelDifference::deserialize(&mut cur)?;

        let mut updates = Vec::new();

        match diff {
            tl::enums::updates::ChannelDifference::Empty(e) => {
                tracing::debug!("[layer] getChannelDifference: empty (pts={})", e.pts);
                self.inner
                    .pts_state
                    .lock()
                    .await
                    .advance_channel(channel_id, e.pts);
            }
            tl::enums::updates::ChannelDifference::ChannelDifference(d) => {
                tracing::debug!(
                    "[layer] getChannelDifference: {} messages, {} updates",
                    d.new_messages.len(),
                    d.other_updates.len()
                );
                self.cache_users_slice_pub(&d.users).await;
                self.cache_chats_slice_pub(&d.chats).await;
                for msg in d.new_messages {
                    updates.push(update::Update::NewMessage(
                        update::IncomingMessage::from_raw(msg).with_client(self.clone()),
                    ));
                }
                for upd in d.other_updates {
                    updates.extend(update::from_single_update_pub(upd));
                }
                self.inner
                    .pts_state
                    .lock()
                    .await
                    .advance_channel(channel_id, d.pts);
            }
            tl::enums::updates::ChannelDifference::TooLong(d) => {
                tracing::warn!(
                    "[layer] getChannelDifference TooLong — replaying messages, resetting pts"
                );
                self.cache_users_slice_pub(&d.users).await;
                self.cache_chats_slice_pub(&d.chats).await;
                for msg in d.messages {
                    updates.push(update::Update::NewMessage(
                        update::IncomingMessage::from_raw(msg).with_client(self.clone()),
                    ));
                }
                self.inner
                    .pts_state
                    .lock()
                    .await
                    .advance_channel(channel_id, 0);
            }
        }

        Ok(updates)
    }

    // ── Sync from server ──────────────────────────────────────────────────

    pub async fn sync_pts_state(&self) -> Result<(), InvocationError> {
        let body = self
            .rpc_call_raw_pub(&tl::functions::updates::GetState {})
            .await?;
        let mut cur = Cursor::from_slice(&body);
        let tl::enums::updates::State::State(s) = tl::enums::updates::State::deserialize(&mut cur)?;
        let mut state = self.inner.pts_state.lock().await;
        state.pts = s.pts;
        state.qts = s.qts;
        state.date = s.date;
        state.seq = s.seq;
        state.touch();
        tracing::debug!(
            "[layer] pts synced: pts={}, qts={}, seq={}",
            s.pts,
            s.qts,
            s.seq
        );
        Ok(())
    }

    // ── Gap-check helpers ─────────────────────────────────────────────────

    /// G-17: Check global pts, buffer during possible-gap window, fetch diff if real gap.
    ///
    /// Fix B2: if a global getDifference is already in-flight (getting_global_diff == true),
    /// buffer the update and return immediately — mirrors grammers' getting_diff_for guard
    /// for Key::Common.  Without this, every simultaneous gap detection spawns a redundant
    /// get_difference(), double-advancing pts and corrupting state.
    pub async fn check_and_fill_gap(
        &self,
        new_pts: i32,
        pts_count: i32,
        upd: Option<update::Update>,
    ) -> Result<Vec<update::Update>, InvocationError> {
        // Fix B2: if a global diff is already in flight, just buffer and bail.
        if self.inner.pts_state.lock().await.getting_global_diff {
            tracing::debug!("[layer] global diff in flight — buffering pts={new_pts}");
            if let Some(u) = upd {
                self.inner.possible_gap.lock().await.push_global(u);
            }
            return Ok(vec![]);
        }

        let result = self
            .inner
            .pts_state
            .lock()
            .await
            .check_pts(new_pts, pts_count);
        match result {
            PtsCheckResult::Ok => {
                // Drain any buffered global updates now that we're in sync,
                // then append the current update (which triggered the Ok).
                let mut buffered = self.inner.possible_gap.lock().await.drain_global();
                self.inner.pts_state.lock().await.advance(new_pts);
                if let Some(u) = upd {
                    buffered.push(u);
                }
                Ok(buffered)
            }
            PtsCheckResult::Gap { expected, got } => {
                // Buffer the update first; only fetch getDifference after the
                // deadline has elapsed (avoids spurious getDifference on every
                // slightly out-of-order update).
                if let Some(u) = upd {
                    self.inner.possible_gap.lock().await.push_global(u);
                }
                let deadline_elapsed = self
                    .inner
                    .possible_gap
                    .lock()
                    .await
                    .global_deadline_elapsed();
                if deadline_elapsed {
                    tracing::warn!(
                        "[layer] global pts gap: expected {expected}, got {got} — getDifference"
                    );
                    let buffered = self.inner.possible_gap.lock().await.drain_global();
                    // get_difference now sets/clears getting_global_diff internally (Fix B2).
                    let mut diff_updates = self.get_difference().await?;
                    // Prepend buffered updates so ordering is maintained.
                    diff_updates.splice(0..0, buffered);
                    Ok(diff_updates)
                } else {
                    tracing::debug!(
                        "[layer] global pts gap: expected {expected}, got {got} — buffering (possible gap)"
                    );
                    Ok(vec![])
                }
            }
            PtsCheckResult::Duplicate => {
                tracing::debug!("[layer] global pts duplicate, discarding");
                Ok(vec![])
            }
        }
    }

    /// G-18: Check qts (secret chat updates) and fill gap if needed.
    pub async fn check_and_fill_qts_gap(
        &self,
        new_qts: i32,
        qts_count: i32,
    ) -> Result<Vec<update::Update>, InvocationError> {
        let result = self
            .inner
            .pts_state
            .lock()
            .await
            .check_qts(new_qts, qts_count);
        match result {
            PtsCheckResult::Ok => {
                self.inner.pts_state.lock().await.advance_qts(new_qts);
                Ok(vec![])
            }
            PtsCheckResult::Gap { expected, got } => {
                tracing::warn!("[layer] qts gap: expected {expected}, got {got} — getDifference");
                self.get_difference().await
            }
            PtsCheckResult::Duplicate => Ok(vec![]),
        }
    }

    /// G-19: Check top-level seq and fill gap if needed.
    pub async fn check_and_fill_seq_gap(
        &self,
        new_seq: i32,
        seq_start: i32,
    ) -> Result<Vec<update::Update>, InvocationError> {
        let result = self
            .inner
            .pts_state
            .lock()
            .await
            .check_seq(new_seq, seq_start);
        match result {
            PtsCheckResult::Ok => {
                self.inner.pts_state.lock().await.advance_seq(new_seq);
                Ok(vec![])
            }
            PtsCheckResult::Gap { expected, got } => {
                tracing::warn!("[layer] seq gap: expected {expected}, got {got} — getDifference");
                self.get_difference().await
            }
            PtsCheckResult::Duplicate => Ok(vec![]),
        }
    }

    /// G-15: Check a per-channel pts, fetch getChannelDifference if there is a gap.
    pub async fn check_and_fill_channel_gap(
        &self,
        channel_id: i64,
        new_pts: i32,
        pts_count: i32,
        upd: Option<update::Update>,
    ) -> Result<Vec<update::Update>, InvocationError> {
        // Fix #4: if a diff is already in flight for this channel, skip — prevents
        // 1 gap from spawning N concurrent getChannelDifference tasks.
        if self
            .inner
            .pts_state
            .lock()
            .await
            .getting_diff_for
            .contains(&channel_id)
        {
            tracing::debug!("[layer] channel {channel_id} diff already in flight, skipping");
            if let Some(u) = upd {
                self.inner
                    .possible_gap
                    .lock()
                    .await
                    .push_channel(channel_id, u);
            }
            return Ok(vec![]);
        }

        let result = self
            .inner
            .pts_state
            .lock()
            .await
            .check_channel_pts(channel_id, new_pts, pts_count);
        match result {
            PtsCheckResult::Ok => {
                let mut buffered = self
                    .inner
                    .possible_gap
                    .lock()
                    .await
                    .drain_channel(channel_id);
                self.inner
                    .pts_state
                    .lock()
                    .await
                    .advance_channel(channel_id, new_pts);
                if let Some(u) = upd {
                    buffered.push(u);
                }
                Ok(buffered)
            }
            PtsCheckResult::Gap { expected, got } => {
                if let Some(u) = upd {
                    self.inner
                        .possible_gap
                        .lock()
                        .await
                        .push_channel(channel_id, u);
                }
                let deadline_elapsed = self
                    .inner
                    .possible_gap
                    .lock()
                    .await
                    .channel_deadline_elapsed(channel_id);
                if deadline_elapsed {
                    tracing::warn!(
                        "[layer] channel {channel_id} pts gap: expected {expected}, got {got} — getChannelDifference"
                    );
                    // Fix #4: mark this channel as having a diff in flight.
                    self.inner
                        .pts_state
                        .lock()
                        .await
                        .getting_diff_for
                        .insert(channel_id);
                    let buffered = self
                        .inner
                        .possible_gap
                        .lock()
                        .await
                        .drain_channel(channel_id);
                    match self.get_channel_difference(channel_id).await {
                        Ok(mut diff_updates) => {
                            // Fix #4: diff complete, allow future gaps to be handled.
                            self.inner
                                .pts_state
                                .lock()
                                .await
                                .getting_diff_for
                                .remove(&channel_id);
                            diff_updates.splice(0..0, buffered);
                            Ok(diff_updates)
                        }
                        // Permanent access errors: remove the channel from pts tracking
                        // entirely (mirrors grammers' approach). The next update for this
                        // channel will have local=0 → PtsCheckResult::Ok, advancing pts
                        // without any gap fill. This breaks the infinite gap→CHANNEL_INVALID
                        // loop that happened when advance_channel kept the stale pts alive.
                        //
                        // Common causes:
                        //   - access_hash not in peer cache (update arrived via updateShort
                        //     which carries no chats list)
                        //   - bot was kicked / channel deleted
                        Err(InvocationError::Rpc(ref e))
                            if e.name == "CHANNEL_INVALID"
                                || e.name == "CHANNEL_PRIVATE"
                                || e.name == "CHANNEL_NOT_MODIFIED" =>
                        {
                            tracing::debug!(
                                "[layer] channel {channel_id}: {} — removing from pts tracking \
                                 (next update treated as first-seen, no gap fill)",
                                e.name
                            );
                            {
                                let mut s = self.inner.pts_state.lock().await;
                                s.getting_diff_for.remove(&channel_id);
                                s.channel_pts.remove(&channel_id); // ← grammers fix: delete, not advance
                            }
                            Ok(buffered)
                        }
                        Err(InvocationError::Deserialize(ref msg)) => {
                            // Unrecognised constructor or parse failure — treat same as
                            // CHANNEL_INVALID: remove from tracking so we don't loop.
                            tracing::debug!(
                                "[layer] channel {channel_id}: deserialize error ({msg}) — \
                                 removing from pts tracking"
                            );
                            {
                                let mut s = self.inner.pts_state.lock().await;
                                s.getting_diff_for.remove(&channel_id);
                                s.channel_pts.remove(&channel_id);
                            }
                            Ok(buffered)
                        }
                        Err(e) => {
                            // Fix #4: also clear on unexpected errors so we don't get stuck.
                            self.inner
                                .pts_state
                                .lock()
                                .await
                                .getting_diff_for
                                .remove(&channel_id);
                            Err(e)
                        }
                    }
                } else {
                    tracing::debug!(
                        "[layer] channel {channel_id} pts gap: expected {expected}, got {got} — buffering"
                    );
                    Ok(vec![])
                }
            }
            PtsCheckResult::Duplicate => {
                tracing::debug!("[layer] channel {channel_id} pts duplicate, discarding");
                Ok(vec![])
            }
        }
    }

    /// G-16: Called periodically (e.g. from keepalive) to fire getDifference
    /// if no update has been received for > 15 minutes.
    ///
    /// Fix B3: also drives per-entry possible-gap deadlines independently of
    /// incoming updates.  Previously the POSSIBLE_GAP_DEADLINE_MS window was
    /// only evaluated when a new incoming update called check_and_fill_gap —
    /// meaning a quiet channel with a real gap would never fire getDifference
    /// until another update arrived.  This matches grammers' check_deadlines()
    /// which scans all LiveEntry.effective_deadline() on every keepalive tick.
    pub async fn check_update_deadline(&self) -> Result<(), InvocationError> {
        // ── existing 15-minute global timeout ──────────────────────────────
        let exceeded = self.inner.pts_state.lock().await.deadline_exceeded();
        if exceeded {
            tracing::info!("[layer] G-16 update deadline exceeded — fetching getDifference");
            let updates = self.get_difference().await?;
            for u in updates {
                if self.inner.update_tx.try_send(u).is_err() {
                    tracing::warn!("[layer] update channel full — dropping diff update");
                }
            }
        }

        // ── Fix B3a: drive global possible-gap deadline ────────────────────
        // If the possible-gap window has expired but no new update has arrived
        // to trigger check_and_fill_gap, fire getDifference from here.
        {
            let gap_expired = self
                .inner
                .possible_gap
                .lock()
                .await
                .global_deadline_elapsed();
            let already = self.inner.pts_state.lock().await.getting_global_diff;
            if gap_expired && !already {
                tracing::debug!("[layer] B3 global possible-gap deadline expired — getDifference");
                let buffered = self.inner.possible_gap.lock().await.drain_global();
                match self.get_difference().await {
                    Ok(mut updates) => {
                        updates.splice(0..0, buffered);
                        for u in updates {
                            if self.inner.update_tx.try_send(u).is_err() {
                                tracing::warn!("[layer] update channel full — dropping gap update");
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("[layer] B3 global gap diff failed: {e}");
                        return Err(e);
                    }
                }
            }
        }

        // ── Fix B3b: drive per-channel possible-gap deadlines ──────────────
        // Collect expired channel IDs up-front to avoid holding the lock across awaits.
        let expired_channels: Vec<i64> = {
            let gap = self.inner.possible_gap.lock().await;
            gap.channel
                .keys()
                .copied()
                .filter(|&id| gap.channel_deadline_elapsed(id))
                .collect()
        };
        for channel_id in expired_channels {
            let already = self
                .inner
                .pts_state
                .lock()
                .await
                .getting_diff_for
                .contains(&channel_id);
            if already {
                continue;
            }
            tracing::debug!(
                "[layer] B3 channel {channel_id} possible-gap deadline expired — getChannelDifference"
            );
            // Mark in-flight before spawning so a racing incoming update can't
            // also spawn a diff for the same channel.
            self.inner
                .pts_state
                .lock()
                .await
                .getting_diff_for
                .insert(channel_id);
            let buffered = self
                .inner
                .possible_gap
                .lock()
                .await
                .drain_channel(channel_id);
            let c = self.clone();
            let utx = self.inner.update_tx.clone();
            tokio::spawn(async move {
                match c.get_channel_difference(channel_id).await {
                    Ok(mut updates) => {
                        c.inner
                            .pts_state
                            .lock()
                            .await
                            .getting_diff_for
                            .remove(&channel_id);
                        updates.splice(0..0, buffered);
                        for u in updates {
                            if utx.try_send(attach_client_to_update(u, &c)).is_err() {
                                tracing::warn!(
                                    "[layer] update channel full — dropping ch gap update"
                                );
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("[layer] B3 channel {channel_id} gap diff failed: {e}");
                        c.inner
                            .pts_state
                            .lock()
                            .await
                            .getting_diff_for
                            .remove(&channel_id);
                    }
                }
            });
        }

        Ok(())
    }
}
