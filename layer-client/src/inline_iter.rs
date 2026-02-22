//! Paginated inline query iterator.
//!
//! Unlike the update stream which delivers live inline queries as they arrive,
//! [`InlineQueryIter`] lets you replay/inspect queries stored in the update
//! buffer.  It is backed by an [`tokio::sync::mpsc`] channel so callers can
//! pull inline queries one at a time instead of blocking on `stream_updates`.

use tokio::sync::mpsc;
use crate::update::{InlineQuery, Update};
use crate::Client;

// ─── InlineQueryIter ─────────────────────────────────────────────────────────

/// Async iterator over incoming inline queries.
///
/// Created by [`Client::iter_inline_queries`].  Each call to [`next`] blocks
/// until the next inline query arrives or the client disconnects.
///
/// # Example
/// ```rust,no_run
/// # async fn f(client: layer_client::Client) {
/// let mut iter = client.iter_inline_queries();
/// while let Some(query) = iter.next().await {
///     println!("Inline query from {}: {:?}", query.user_id, query.query());
/// }
/// # }
/// ```
pub struct InlineQueryIter {
    rx: mpsc::UnboundedReceiver<InlineQuery>,
}

impl InlineQueryIter {
    /// Wait for the next inline query.  Returns `None` when the stream ends.
    pub async fn next(&mut self) -> Option<InlineQuery> {
        self.rx.recv().await
    }
}

// ─── Client extension ─────────────────────────────────────────────────────────

impl Client {
    /// Return an [`InlineQueryIter`] that yields every incoming inline query.
    ///
    /// Internally this spawns the same update loop as [`stream_updates`] but
    /// filters for [`Update::InlineQuery`] events only.
    ///
    /// [`stream_updates`]: Client::stream_updates
    pub fn iter_inline_queries(&self) -> InlineQueryIter {
        let (tx, rx) = mpsc::unbounded_channel();
        let client   = self.clone();

        tokio::spawn(async move {
            let mut stream = client.stream_updates();
            loop {
                match stream.next().await {
                    Some(Update::InlineQuery(q)) => {
                        if tx.send(q).is_err() { break; }
                    }
                    Some(_) => {} // ignore other updates
                    None    => break,
                }
            }
        });

        InlineQueryIter { rx }
    }
}
