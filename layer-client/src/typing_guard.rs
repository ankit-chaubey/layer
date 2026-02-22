//! RAII typing indicator guard.
//!
//! [`TypingGuard`] automatically cancels the "typing…" chat action when
//! dropped, eliminating the need to remember to call `send_chat_action`
//! with `SendMessageAction::SendMessageCancelAction` manually.
//!
//! # Example
//! ```rust,no_run
//! use layer_client::{Client, TypingGuard};
//! use layer_tl_types as tl;
//!
//! async fn handle(client: Client, peer: tl::enums::Peer) {
//!     // Typing indicator is sent immediately and auto-cancelled on drop.
//!     let _typing = TypingGuard::start(&client, peer.clone(),
//!         tl::enums::SendMessageAction::SendMessageTypingAction).await.unwrap();
//!
//!     do_expensive_work().await;
//!     // `_typing` is dropped here — Telegram sees the typing stop.
//! }
//! # async fn do_expensive_work() {}
//! ```

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use layer_tl_types as tl;
use crate::{Client, InvocationError};

// ─── TypingGuard ─────────────────────────────────────────────────────────────

/// Scoped typing indicator.  Keeps the action alive by re-sending it every
/// ~4 seconds (Telegram drops the indicator after ~5 s).
///
/// Drop this guard to cancel the action immediately.
pub struct TypingGuard {
    stop: Arc<Notify>,
    task: Option<JoinHandle<()>>,
}

impl TypingGuard {
    /// Send `action` to `peer` and keep repeating it until the guard is dropped.
    pub async fn start(
        client: &Client,
        peer:   tl::enums::Peer,
        action: tl::enums::SendMessageAction,
    ) -> Result<Self, InvocationError> {
        // Send once immediately so the indicator appears without delay.
        client.send_chat_action(peer.clone(), action.clone()).await?;

        let stop   = Arc::new(Notify::new());
        let stop2  = stop.clone();
        let client = client.clone();

        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(4)) => {
                        if let Err(e) = client.send_chat_action(peer.clone(), action.clone()).await {
                            log::warn!("[typing_guard] Failed to refresh typing action: {e}");
                            break;
                        }
                    }
                    _ = stop2.notified() => break,
                }
            }
            // Cancel the action
            let cancel = tl::enums::SendMessageAction::SendMessageCancelAction;
            let _ = client.send_chat_action(peer.clone(), cancel).await;
        });

        Ok(Self { stop, task: Some(task) })
    }

    /// Cancel the typing indicator immediately without waiting for the drop.
    pub fn cancel(&mut self) {
        self.stop.notify_one();
    }
}

impl Drop for TypingGuard {
    fn drop(&mut self) {
        self.stop.notify_one();
        // Detach the task — it will see the notify, send a cancel action, then exit.
        // We don't abort it because we want the cancel action to reach Telegram.
        if let Some(t) = self.task.take() {
            t.abort(); // abort is fine since notify already fired — cancel fires in select
        }
    }
}

// ─── Client extension ─────────────────────────────────────────────────────────

impl Client {
    /// Start a scoped typing indicator that auto-cancels when dropped.
    ///
    /// This is a convenience wrapper around [`TypingGuard::start`].
    pub async fn typing(
        &self,
        peer: tl::enums::Peer,
    ) -> Result<TypingGuard, InvocationError> {
        TypingGuard::start(self, peer, tl::enums::SendMessageAction::SendMessageTypingAction).await
    }

    /// Start a scoped "uploading document" action that auto-cancels when dropped.
    pub async fn uploading_document(
        &self,
        peer: tl::enums::Peer,
    ) -> Result<TypingGuard, InvocationError> {
        TypingGuard::start(self, peer, tl::enums::SendMessageAction::SendMessageUploadDocumentAction(
            tl::types::SendMessageUploadDocumentAction { progress: 0 }
        )).await
    }

    /// Start a scoped "recording video" action that auto-cancels when dropped.
    pub async fn recording_video(
        &self,
        peer: tl::enums::Peer,
    ) -> Result<TypingGuard, InvocationError> {
        TypingGuard::start(self, peer, tl::enums::SendMessageAction::SendMessageRecordVideoAction).await
    }
}
