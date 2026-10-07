//! Async transport trait for zero-cost async VISCA communication.
//!
//! This trait provides zero-overhead async transport via RPITIT methods that
//! return unboxed futures with explicit `Send` guarantees.

use std::future::Future;

use crate::{
    transport::{AddressingMode, ReceiveOutcome, SendSemantics},
    Error,
};

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
///     fn recv_into<'a>(
///         &'a mut self,
///         dst: &'a mut [u8],
///     ) -> impl Future<Output = Result<ReceiveOutcome, Error>> + Send {
///         // Nothing arrived before this transport's own idle timeout.
///         async move { Err(Error::io_timeout()) }
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
    ///   genuinely dead socket from [`AsyncTransport::recv_into`],
    ///   which is the side the runtime treats as authoritative about session
    ///   death.
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

    /// Receive raw bytes from the device into `dst`, reporting whether
    /// everything received fitted.
    ///
    /// This method reads the next available chunk of bytes from the transport
    /// into the provided buffer. It may return partial frames, complete
    /// frames, or multiple frames. The runtime is responsible for aggregating
    /// chunks and extracting frames.
    ///
    /// # Receive outcome
    ///
    /// The [`ReceiveOutcome`] is the transport's own statement about what it
    /// copied; the runtime never infers it from the byte count. See
    /// [`ReceiveOutcome`] for the datagram policy.
    ///
    /// - A byte stream always reports [`ReceiveOutcome::complete`]: a short
    ///   read is routine and the runtime buffers the remainder.
    /// - A datagram transport reports [`ReceiveOutcome::complete`] only when
    ///   the whole datagram fitted, [`ReceiveOutcome::truncated`] when its
    ///   runtime or OS reported a discarded tail, and
    ///   [`ReceiveOutcome::possibly_truncated`] when it filled `dst` and
    ///   cannot tell. The runtime discards a datagram that is not complete as
    ///   one malformed input and keeps the session; it never decodes its
    ///   prefix. A byte stream that reports one ends the session, because
    ///   bytes it consumed were lost.
    /// - A datagram transport never reports zero bytes for an empty datagram
    ///   (zero bytes is end of stream): it skips the empty datagram and keeps
    ///   reading within the same deadline. One that reports
    ///   [`ReceiveOutcome::possibly_truncated`] for every exact fill needs a
    ///   `recv_buffer_size` of at least
    ///   [`BufferConfig::MIN_RECV_BUFFER_SIZE`](crate::transport::BufferConfig::MIN_RECV_BUFFER_SIZE)
    ///   ` + 1`; see [`ReceiveOutcome`].
    ///
    /// # Error contract
    ///
    /// The runtime never guesses: the value this method returns decides whether
    /// the session lives, and whether every command still waiting for its ACK
    /// is retransmitted. A failed read must consume nothing, so that the
    /// runtime's framing state stays intact.
    ///
    /// - `Ok(outcome)` with a non-zero [`ReceiveOutcome::copied_len`] — bytes
    ///   were read. Returning fewer bytes than a whole frame is normal and is
    ///   not an error.
    /// - `Ok(ReceiveOutcome::complete(0))` — end of stream: the peer closed.
    ///   The runtime ends the session with [`Error::ConnectionClosed`]. Never
    ///   report zero bytes to mean "no data yet"; that is the one signal
    ///   reserved for EOF. Returning `Err(Error::ConnectionClosed)` for EOF is
    ///   also accepted and ends the session the same way.
    /// - `Err(Error::io_timeout())` — an idle read timeout expired and no bytes
    ///   arrived. This is the recommended shape for a transport that must not
    ///   block indefinitely. The runtime treats it as "no data": the session
    ///   lives, framing state is untouched, and no request's retry budget is
    ///   spent. The raw I/O spellings [`std::io::ErrorKind::WouldBlock`] and
    ///   [`std::io::ErrorKind::Interrupted`] wrapped in [`Error::Io`] are
    ///   normalized to the same meaning. Any [`Error::Timeout`] is read the same
    ///   way. Use [`Error::io_timeout`], not a raw
    ///   [`std::io::ErrorKind::TimedOut`], for an application-owned idle timer:
    ///   a connected TCP socket can report the latter when OS keepalive
    ///   exhausts, and the runtime treats that as session death.
    /// - A session-fatal error — any error for which
    ///   [`Error::requires_new_session`] is true, plus [`Error::Io`] carrying
    ///   `TimedOut`, `ConnectionReset`, `ConnectionAborted`, `BrokenPipe`,
    ///   `UnexpectedEof` or `NotConnected`. The runtime ends the session as
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
    /// as if it had returned `Err(Error::io_timeout())`. A read that consumed nothing
    /// on cancellation keeps framing state intact, so implementations should be
    /// cancellation-safe; futures built from the standard async socket readers
    /// already are. A transport that *does* enforce its own internal timeout and
    /// returns `Err(Error::io_timeout())` also works and composes with this bound.
    ///
    /// # Forwarding wrappers
    ///
    /// A transport that wraps another must forward this method to the inner
    /// transport, which is the authority on its own datagram boundaries.
    fn recv_into<'a>(
        &'a mut self,
        dst: &'a mut [u8],
    ) -> impl Future<Output = Result<ReceiveOutcome, Error>> + Send;

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
