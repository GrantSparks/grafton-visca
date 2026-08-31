//! Async transport trait for zero-cost async VISCA communication.
//!
//! This trait provides zero-overhead async transport via RPITIT methods that
//! return unboxed futures with explicit `Send` guarantees.

use std::future::Future;

use crate::{
    transport::{AddressingMode, SendSemantics},
    Error,
};

/// Result of one transport receive with datagram-boundary information.
///
/// A byte stream always reports [`ReceiveOutcome::Complete`]. Datagram
/// transports must not hand bytes to the protocol decoder unless the outcome
/// is complete: a valid-looking prefix is not a complete datagram.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "a receive outcome must be classified before its bytes are decoded"]
pub enum ReceiveOutcome {
    /// The complete payload was copied into the caller's buffer.
    Complete {
        /// Number of payload bytes copied.
        bytes: usize,
    },
    /// The runtime/OS positively reported that the datagram was truncated.
    Truncated {
        /// Number of payload bytes copied before the tail was discarded.
        copied: usize,
    },
    /// A legacy datagram receiver filled the supplied buffer but cannot report
    /// whether a tail was discarded.
    ///
    /// This is deliberately rejected just like [`Self::Truncated`]. It keeps
    /// existing custom transports source-compatible while making the fallback
    /// safe: implementations that can distinguish an exact fit from
    /// truncation should override [`AsyncTransport::recv_into_with_outcome`].
    PossiblyTruncated {
        /// Number of payload bytes copied.
        copied: usize,
    },
}

impl ReceiveOutcome {
    /// Number of payload bytes copied into the supplied buffer.
    pub const fn copied_len(self) -> usize {
        match self {
            Self::Complete { bytes }
            | Self::Truncated { copied: bytes }
            | Self::PossiblyTruncated { copied: bytes } => bytes,
        }
    }

    /// Whether all payload bytes are known to have been copied.
    pub const fn is_complete(self) -> bool {
        matches!(self, Self::Complete { .. })
    }
}

/// Async transport for VISCA communication.
///
/// This trait uses return-position impl trait in traits (RPITIT) with explicit
/// `+ Send` bounds to ensure futures can be safely sent across threads.
///
/// # Example
///
/// ```rust,ignore
/// use grafton_visca::transport::AsyncTransport;
/// struct MyAsyncTransport { /* ... */ }
///
/// impl AsyncTransport for MyAsyncTransport {
///     fn send(&mut self, _bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
///         async move { Ok(()) }
///     }
///
///     fn recv_into<'a>(&'a mut self, dst: &'a mut [u8]) -> impl Future<Output = Result<usize, Error>> + Send {
///         async move { Ok(0) }
///     }
/// }
/// ```
pub trait AsyncTransport: Send {
    /// Send raw bytes to the device.
    ///
    /// This method sends the provided bytes over the transport and returns
    /// when the bytes have been written to the underlying transport.
    ///
    /// # Error contract
    ///
    /// What a send failure costs is decided by [`AsyncTransport::send_semantics`],
    /// not by the error value:
    ///
    /// - [`SendSemantics::Stream`]: the byte-stream position is now unknowable,
    ///   so the runtime poisons the session. Every request in flight fails with
    ///   [`Error::StreamPoisoned`] carrying this error's text.
    /// - [`SendSemantics::Datagram`]: only the request being written fails and
    ///   the session keeps running. Because the session survives, the runtime
    ///   normalizes a session-fatal error value (any error for which
    ///   [`Error::requires_new_session`] is true, such as
    ///   [`Error::ConnectionClosed`]) into a plain per-request
    ///   [`Error::TransportError`]: a caller must never be told to open a new
    ///   session by an error raised on one that is still running. Report a
    ///   genuinely dead socket from [`AsyncTransport::recv_into`], which is the
    ///   side the runtime treats as authoritative about session death.
    ///
    /// # Timeout and cancellation
    ///
    /// The async runtime bounds every write with the session's configured
    /// `write_timeout` (the `TransportConfig` knob), because the runtime-agnostic
    /// transports carry no timer of their own. If the write outlasts that budget
    /// the runtime drops this future and treats the write as failed under the
    /// semantics above — a stream write that can no longer be confirmed poisons
    /// the session; a datagram write fails only its own request. Implementations
    /// should therefore be cancellation-safe (dropping a partly-issued write must
    /// not corrupt later writes beyond what the stream/datagram contract already
    /// allows); futures built from the standard async socket writers already are.
    fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send;

    /// Receive raw bytes from the device into the provided buffer.
    ///
    /// This method reads the next available chunk of bytes from the transport
    /// into the provided buffer and returns the number of bytes read.
    /// It may return partial frames, complete frames, or multiple frames.
    /// The runtime is responsible for aggregating chunks and extracting frames.
    ///
    /// # Error contract
    ///
    /// The runtime never guesses: the value this method returns decides whether
    /// the session lives, and whether every command still waiting for its ACK
    /// is retransmitted. An ordinary failed read must consume nothing, so that
    /// the runtime's framing state stays intact. The one exception is a UDP
    /// `Error::ResponseTooLarge`: that legacy spelling means the transport
    /// deliberately discarded one consumed over-size datagram. Owners use
    /// [`AsyncTransport::recv_into_with_outcome`] to preserve that distinction.
    ///
    /// - `Ok(n)` with `n > 0` — bytes were read. Returning fewer bytes than a
    ///   whole frame is normal and is not an error; the runtime buffers the
    ///   remainder until a later read completes the frame.
    /// - `Ok(0)` — end of stream: the peer closed. The runtime ends the session
    ///   with [`Error::ConnectionClosed`]. Never return `Ok(0)` to mean "no data
    ///   yet"; that is the one signal reserved for EOF. Returning
    ///   `Err(Error::ConnectionClosed)` for EOF is also accepted and ends the
    ///   session the same way, but `Ok(0)` is canonical.
    /// - `Err(Error::Timeout)` — an idle read timeout expired and no bytes
    ///   arrived. This is the recommended shape for a transport that must not
    ///   block indefinitely. The runtime treats it as "no data": the session
    ///   lives, framing state is untouched, and no request's retry budget is
    ///   spent. The raw I/O spellings [`std::io::ErrorKind::TimedOut`],
    ///   [`std::io::ErrorKind::WouldBlock`] and
    ///   [`std::io::ErrorKind::Interrupted`] wrapped in [`Error::Io`] are
    ///   normalized to the same meaning.
    /// - A session-fatal error — any error for which
    ///   [`Error::requires_new_session`] is true, plus [`Error::Io`] carrying
    ///   `ConnectionReset`, `ConnectionAborted`, `BrokenPipe`, `UnexpectedEof`
    ///   or `NotConnected`. The runtime ends the session as
    ///   [`Error::ConnectionClosed`] and retains the transport cause's text in
    ///   its reason. A failed read consumed nothing, so it is not a stream
    ///   framing poison; [`Error::StreamPoisoned`] is reserved for an
    ///   unknowable stream position caused by framing loss or a failed write.
    /// - Any other error is a *transient* fault: the read failed but the
    ///   connection may still be usable — a UDP `recv` reporting ECONNREFUSED
    ///   after an ICMP port-unreachable is the canonical case. The engine may
    ///   retransmit sequence-correlated Sony commands still waiting for their
    ///   ACK under each command's retry policy. A raw command awaiting ACK has
    ///   no sequence key, so it is left to its own ACK deadline and is never
    ///   replayed merely because of this fault. Faults that repeat with no
    ///   successful read in between escalate: the pause between reads grows,
    ///   and after roughly two seconds of uninterrupted failure the runtime
    ///   ends the session with a normalized `ConnectionClosed` cause. Do not
    ///   use this class for idle timeouts.
    ///
    /// # Timeout and cancellation
    ///
    /// A transport may block until data arrives; it need not implement its own
    /// read timeout. The async runtime bounds every read with the session's
    /// configured `read_timeout` (the `TransportConfig` knob) and, if it elapses,
    /// drops this future and treats the read as an idle no-data receive — exactly
    /// as if it had returned `Err(Error::Timeout)`. A read that consumed nothing
    /// on cancellation keeps framing state intact, so implementations should be
    /// cancellation-safe; futures built from the standard async socket readers
    /// already are. A transport that *does* enforce its own internal timeout and
    /// returns `Err(Error::Timeout)` also works and composes with this bound.
    fn recv_into<'a>(
        &'a mut self,
        dst: &'a mut [u8],
    ) -> impl Future<Output = Result<usize, Error>> + Send;

    /// Receive raw bytes together with whether a datagram fitted in `dst`.
    ///
    /// This supplements [`AsyncTransport::recv_into`] without changing its
    /// signature. Existing implementations remain source-compatible: the
    /// default calls `recv_into`, and a full destination buffer from a
    /// datagram transport is conservatively returned as
    /// [`ReceiveOutcome::PossiblyTruncated`]. A custom datagram transport that
    /// can observe its runtime's truncation indication should override this
    /// method, returning [`ReceiveOutcome::Complete`] for a known exact fit
    /// and [`ReceiveOutcome::Truncated`] when a tail was discarded.
    ///
    /// Wrappers must forward this method to their inner transport, just as
    /// they forward [`AsyncTransport::recv_into`] and
    /// [`AsyncTransport::send_semantics`].
    fn recv_into_with_outcome<'a>(
        &'a mut self,
        dst: &'a mut [u8],
    ) -> impl Future<Output = Result<ReceiveOutcome, Error>> + Send {
        async move {
            let capacity = dst.len();
            let bytes = self.recv_into(dst).await?;
            if self.send_semantics() == SendSemantics::Datagram && bytes == capacity {
                Ok(ReceiveOutcome::PossiblyTruncated { copied: bytes })
            } else {
                Ok(ReceiveOutcome::Complete { bytes })
            }
        }
    }

    /// Return a side-effect-free hint for the transport's VISCA addressing mode.
    ///
    /// Multi-target sessions require an explicit [`AddressingMode::Serial`]
    /// declaration because serial replies carry a camera source while IP
    /// replies do not. Custom transports default to `None`, which conservatively
    /// rejects multi-target startup before configuration is read. Single-target
    /// sessions do not require a hint. The hint only authorizes preflight;
    /// `TransportConfig::addressing` remains the runtime's framing source.
    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        None
    }

    /// Query the transport's send semantics.
    ///
    /// This method returns whether the transport uses stream or datagram semantics,
    /// which determines how the runtime handles send failures:
    ///
    /// - [`SendSemantics::Stream`]: On send failure/timeout, the transport is
    ///   "poisoned" and the runtime stops using it to prevent protocol desync.
    ///
    /// - [`SendSemantics::Datagram`]: On send failure, only the affected command
    ///   fails; the transport continues operating for subsequent commands.
    ///
    /// # Default Implementation
    ///
    /// Returns [`SendSemantics::Stream`] by default, which is the safer choice
    /// for unknown transport types. Datagram transports (UDP) should override
    /// this to return [`SendSemantics::Datagram`].
    ///
    /// # Forwarding wrappers
    ///
    /// A transport that wraps another (for example an enum that dispatches to a
    /// TCP, UDP, or serial inner transport) **must** forward this method to the
    /// inner transport, exactly as it forwards [`AsyncTransport::send`] and
    /// [`AsyncTransport::recv_into`]. Unlike a missing `match` arm, an
    /// unforwarded `send_semantics` does not fail to compile — it silently falls
    /// back to this `Stream` default, which is wrong for any datagram inner
    /// transport and demotes it to the stream-poison policy. The inner transport
    /// is the authority on its own semantics.
    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Stream
    }
}
