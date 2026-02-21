//! Pluggable transport layer.
//!
//! Implement [`Transport`] over TCP, WebSocket, or any other byte-stream
//! protocol to get MTProto message framing for free.

/// A full-duplex byte-stream transport.
///
/// Implementations are expected to handle their own buffering.
/// The MTProto layer operates on complete *framed* messages.
pub trait Transport {
    /// The error type returned by read/write operations.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Send raw bytes to the remote.
    fn send(&mut self, data: &[u8]) -> Result<(), Self::Error>;

    /// Receive the next complete MTProto packet from the remote.
    ///
    /// Implementations should block until a full packet is available.
    fn recv(&mut self) -> Result<Vec<u8>, Self::Error>;
}

// ─── Abridged framing ─────────────────────────────────────────────────────────

/// Wraps a `Transport` and applies the [MTProto Abridged] framing.
///
/// Abridged is the simplest framing: send `0xef` on first connection,
/// then each packet is `[length/4 as 1 or 4 bytes][payload]`.
///
/// [MTProto Abridged]: https://core.telegram.org/mtproto/mtproto-transports#abridged
pub struct AbridgedTransport<T: Transport> {
    inner: T,
    init_sent: bool,
}

impl<T: Transport> AbridgedTransport<T> {
    /// Wrap an existing transport in abridged framing.
    pub fn new(inner: T) -> Self {
        Self { inner, init_sent: false }
    }

    /// Send a plaintext message applying abridged length prefix.
    pub fn send_message(&mut self, data: &[u8]) -> Result<(), T::Error> {
        if !self.init_sent {
            self.inner.send(&[0xef])?;
            self.init_sent = true;
        }

        let len = data.len() / 4;
        let header: Vec<u8> = if len < 127 {
            vec![len as u8]
        } else {
            vec![
                0x7f,
                (len & 0xff) as u8,
                ((len >> 8) & 0xff) as u8,
                ((len >> 16) & 0xff) as u8,
            ]
        };

        self.inner.send(&header)?;
        self.inner.send(data)
    }

    /// Receive the next abridged-framed message.
    pub fn recv_message(&mut self) -> Result<Vec<u8>, T::Error> {
        // The inner transport delivers complete raw packets;
        // callers can also call inner.recv() directly after de-framing.
        self.inner.recv()
    }

    /// Access the underlying transport.
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}
