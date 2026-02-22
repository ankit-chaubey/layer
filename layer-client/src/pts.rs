//! Update gap detection and recovery via `updates.getDifference`.
//!
//! The Telegram MTProto protocol assigns a monotonically-increasing sequence
//! number called **pts** (and **qts** for secret chats, **seq** for the
//! combined updates container) to each update.  If the client misses updates
//! (due to a disconnect, lag, or packet loss) the pts will jump forward.  This
//! module tracks the current pts and fetches any missed updates via
//! `updates.getDifference` when a gap is detected.

use layer_tl_types as tl;
use layer_tl_types::{Cursor, Deserializable};

use crate::{Client, InvocationError, update};

// ─── PtsState ─────────────────────────────────────────────────────────────────

/// Tracks MTProto sequence numbers so we can detect and fill update gaps.
#[derive(Default, Debug, Clone)]
pub struct PtsState {
    /// Main sequence counter (messages, channels).
    pub pts: i32,
    /// Secondary counter for secret chats.
    pub qts: i32,
    /// Date of the last known update (Unix timestamp).
    pub date: i32,
    /// Combined updates sequence.
    pub seq: i32,
}

impl PtsState {
    /// Create a new PtsState from the current server state.
    pub fn from_server_state(state: &tl::types::updates::State) -> Self {
        Self {
            pts:  state.pts,
            qts:  state.qts,
            date: state.date,
            seq:  state.seq,
        }
    }

    /// Returns true if `new_pts == self.pts + pts_count` (no gap).
    pub fn check_pts(&self, new_pts: i32, pts_count: i32) -> PtsCheckResult {
        let expected = self.pts + pts_count;
        if new_pts == expected {
            PtsCheckResult::Ok
        } else if new_pts > expected {
            PtsCheckResult::Gap { expected, got: new_pts }
        } else {
            PtsCheckResult::Duplicate
        }
    }

    /// Apply a confirmed pts advance.
    pub fn advance(&mut self, new_pts: i32) {
        if new_pts > self.pts { self.pts = new_pts; }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PtsCheckResult {
    /// pts is in order — process the update.
    Ok,
    /// pts jumped forward — there is a gap; must call getDifference.
    Gap { expected: i32, got: i32 },
    /// pts is in the past — update already seen; discard.
    Duplicate,
}

// ─── Client methods ───────────────────────────────────────────────────────────

impl Client {
    /// Fetch and apply any missed updates since the last known pts.
    ///
    /// This should be called after reconnection to close any update gap.
    /// Returns the updates that were missed.
    pub async fn get_difference(&self) -> Result<Vec<update::Update>, InvocationError> {
        let (pts, qts, date) = {
            let state = self.inner.pts_state.lock().await;
            (state.pts, state.qts, state.date)
        };

        if pts == 0 {
            // No state yet; fetch current state from server first.
            self.sync_pts_state().await?;
            return Ok(vec![]);
        }

        log::info!("[layer] getDifference (pts={pts}, qts={qts}, date={date}) …");

        let req = tl::functions::updates::GetDifference {
            pts,
            pts_limit:       None,
            pts_total_limit: None,
            date,
            qts,
            qts_limit:       None,
        };

        let body    = self.rpc_call_raw_pub(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        let diff    = tl::enums::updates::Difference::deserialize(&mut cur)?;

        let mut updates = Vec::new();
        match diff {
            tl::enums::updates::Difference::Empty(e) => {
                // No new updates; fast-forward our state
                let mut state = self.inner.pts_state.lock().await;
                state.date = e.date;
                state.seq  = e.seq;
                log::debug!("[layer] getDifference: empty (seq={})", e.seq);
            }
            tl::enums::updates::Difference::Difference(d) => {
                log::info!("[layer] getDifference: {} messages, {} updates",
                    d.new_messages.len(), d.other_updates.len());
                // Cache users and chats
                self.cache_users_slice_pub(&d.users).await;
                self.cache_chats_slice_pub(&d.chats).await;
                // Emit new messages as updates
                for msg in d.new_messages {
                    updates.push(update::Update::NewMessage(
                        update::IncomingMessage::from_raw(msg)
                    ));
                }
                // Emit other updates
                for upd in d.other_updates {
                    updates.extend(update::from_single_update_pub(upd));
                }
                // Advance pts state using the returned state
                let ns = match d.state {
                    tl::enums::updates::State::State(s) => s,
                };
                let mut state = self.inner.pts_state.lock().await;
                *state = PtsState::from_server_state(&ns);
            }
            tl::enums::updates::Difference::Slice(d) => {
                log::info!("[layer] getDifference slice: {} messages, {} updates",
                    d.new_messages.len(), d.other_updates.len());
                self.cache_users_slice_pub(&d.users).await;
                self.cache_chats_slice_pub(&d.chats).await;
                for msg in d.new_messages {
                    updates.push(update::Update::NewMessage(
                        update::IncomingMessage::from_raw(msg)
                    ));
                }
                for upd in d.other_updates {
                    updates.extend(update::from_single_update_pub(upd));
                }
                // Slice has intermediate_state
                let ns = match d.intermediate_state {
                    tl::enums::updates::State::State(s) => s,
                };
                let mut state = self.inner.pts_state.lock().await;
                *state = PtsState::from_server_state(&ns);
            }
            tl::enums::updates::Difference::TooLong(d) => {
                log::warn!("[layer] getDifference: TooLong (pts={}) — re-syncing state", d.pts);
                // Jump to the new pts and re-sync
                let mut state = self.inner.pts_state.lock().await;
                state.pts = d.pts;
                drop(state);
                self.sync_pts_state().await?;
            }
        }

        Ok(updates)
    }

    /// Fetch the current server update state and store it locally.
    pub async fn sync_pts_state(&self) -> Result<(), InvocationError> {
        let req  = tl::functions::updates::GetState {};
        let body = self.rpc_call_raw_pub(&req).await?;
        let mut cur = Cursor::from_slice(&body);
        let state = match tl::enums::updates::State::deserialize(&mut cur)? {
            tl::enums::updates::State::State(s) => s,
        };
        let mut pts_state = self.inner.pts_state.lock().await;
        *pts_state = PtsState::from_server_state(&state);
        log::info!("[layer] pts synced: pts={}, qts={}, seq={}", state.pts, state.qts, state.seq);
        Ok(())
    }

    /// Check for update gaps and fill them before processing an update with the given pts.
    ///
    /// Returns any catch-up updates that were missed.
    pub async fn check_and_fill_gap(
        &self,
        new_pts:   i32,
        pts_count: i32,
    ) -> Result<Vec<update::Update>, InvocationError> {
        let result = {
            let state = self.inner.pts_state.lock().await;
            state.check_pts(new_pts, pts_count)
        };

        match result {
            PtsCheckResult::Ok => {
                let mut state = self.inner.pts_state.lock().await;
                state.advance(new_pts);
                Ok(vec![])
            }
            PtsCheckResult::Gap { expected, got } => {
                log::warn!("[layer] pts gap detected: expected {expected}, got {got} — fetching difference");
                self.get_difference().await
            }
            PtsCheckResult::Duplicate => {
                log::debug!("[layer] pts duplicate, discarding update");
                Ok(vec![])
            }
        }
    }
}
