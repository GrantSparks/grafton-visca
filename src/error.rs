//! Error classification and recovery decisions.
//!
//! `Error::kind()` is a coarse reporting category, `is_retryable()` describes
//! whether a new bounded attempt can be reasonable, and
//! `requires_new_session()` answers only whether this error positively proves
//! that the current session is unusable. None of those predicates is a
//! perpetual retry policy. In particular, a camera can keep a socket open while
//! producing no VISCA frames; use
//! [`MetricsSnapshot::received_frames`](crate::MetricsSnapshot::received_frames)
//! around an application heartbeat and impose an application-owned silence
//! threshold.
//!
//! ## Owner failure matrix
//!
//! This is the canonical event × envelope × transport mapping. "Any" includes
//! both blocking and async owners; execution mode does not change a verdict.
//!
//! | Event | Envelope | Transport | Public outcome | `requires_new_session()` | Application response |
//! | --- | --- | --- | --- | --- | --- |
//! | Observer or inquiry/response deadline expires without correlation ambiguity | Any | Any | [`Error::Timeout`] | `false` | Bound any new logical attempt. A timeout alone is not liveness proof. |
//! | A sent command's ACK/completion becomes unconfirmable under the default policy | Raw | Datagram or stream | [`Error::UnsequencedCommandUnconfirmed`] ([`ErrorKind::Unconfirmed`]) | `false` | Never replay blindly; reconcile that command's camera effect. Whole and fragmented late bytes have the same verdict: a retained stream prefix gets a bounded grace, then is discarded as malformed rather than poisoning by segmentation. |
//! | A recorded cancellation cannot be resolved before its correlation deadline | Sony, or raw with strict policy off | Datagram or stream | [`Error::CancellationUnconfirmed`] ([`ErrorKind::Unconfirmed`]) | `false` | Reconcile the original command; cancellation was requested, not proven. |
//! | Raw command/cancellation uncertainty under strict policy | Raw | Datagram or stream | [`Error::StreamPoisoned`] | `true` | Replace the session, re-query state, and restore deliberately. |
//! | Request or cancellation send fails | Any | Datagram | The transport error (a terminal-looking custom error is normalized to [`Error::TransportError`]) | `false` | Treat it as this transmission's failure; the receive side remains the authority on session death. |
//! | Request or cancellation send fails after unknown stream progress | Any | Stream | [`Error::StreamPoisoned`] | `true` | Replace the session; stream position may be unknowable. |
//! | Fatal receive closure, including EOF/reset/broken pipe | Any | Datagram or stream | [`Error::ConnectionClosed`] | `true` | Replace the session and re-query state. |
//! | Transient receive fault or an idle/no-data read | Any | Datagram or stream | No immediate public failure; request policy/deadlines continue | n/a | Keep driving the session; use a bounded application heartbeat for silent peers. |
//! | Framer overflow or unrecoverable discard/resynchronization failure | Any | Stream | [`Error::StreamPoisoned`] | `true` | Replace the session. |
//! | Blocking owner re-entry, or an operation-handle submission that cannot win its immediate first-dispatch boundary | Any | Any | [`Error::TransportBusy`] | `false` | Serialize or back off this blocking caller; do not reconnect. |
//! | Local request admission is full | Any | Any | [`Error::RuntimeQueueFull`] | `false` | Back off until admission capacity is available. |
//! | Camera returns a conclusive protocol rejection | Any | Any | The exact VISCA error variant | `false` | Apply the variant's retry policy; camera state and socket routing remain authoritative. |
//! | Application closes the owner | Any | Any | [`Error::RuntimeShutdown`] | `false` | Reconnect only if the application intends to start another session. |

use thiserror::Error as ThisError;

use std::{borrow::Cow, convert::Infallible, io, sync::Arc, time::Duration};

/// Custom result type for VISCA operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Categorized error kinds for structured error handling.
///
/// This enum provides a high-level categorization of errors to enable
/// consistent retry logic and error handling across the library.
///
/// # Example
/// ```rust
/// use grafton_visca::{Error, ErrorKind};
///
/// fn handle_error(error: Error) {
///     match error.kind() {
///         ErrorKind::Timeout => println!("Operation timed out"),
///         ErrorKind::Cancelled => println!("Operation was cancelled"),
///         ErrorKind::BufferFull => println!("Camera buffer full, retry later"),
///         _ => println!("Other error: {}", error),
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// Operation timed out before completion.
    Timeout,

    /// Operation was cancelled by user request.
    Cancelled,

    /// A transmitted command or cancellation has an unknowable outcome.
    ///
    /// The affected request is finished, but the session remains usable by
    /// default. Reconcile camera state before deciding whether to submit the
    /// logical operation again.
    Unconfirmed,

    /// Camera's command buffer is full (retryable).
    BufferFull,

    /// One transport operation failed while the session remains usable.
    ///
    /// Retry the failed logical operation under its own bounded policy. This
    /// category is deliberately distinct from [`Self::IoClosed`]: a live
    /// session must never tell a caller to replace itself for one isolated
    /// transmission failure.
    Transport,

    /// Command cannot be executed in current state.
    NotExecutable,

    /// Connection was closed or lost.
    ///
    /// This category deliberately covers every unusable-session condition,
    /// including the deliberate [`Error::RuntimeShutdown`]. It therefore does
    /// not by itself say whether a replacement session is needed. Use
    /// [`Error::requires_new_session()`] for that classification instead of
    /// matching this kind or individual variants.
    ///
    /// Do not automatically replay a command whose completion is uncertain,
    /// because it may have reached the camera before the connection failed.
    IoClosed,

    /// Connection was refused.
    IoRefused,

    /// Protocol-level error (malformed response, etc.).
    Protocol,

    /// Feature or command not supported by camera.
    Unsupported,

    /// Invalid parameter or out of range value.
    InvalidParameter,

    /// The camera or local owner is temporarily unable to accept more work.
    Busy,

    /// Other unspecified error.
    Other,
}

/// VISCA protocol error type.
///
/// Provides comprehensive error handling for all VISCA operations.
/// The VISCA protocol has a well-defined set of error conditions
/// that map directly to camera responses and communication failures.
///
/// # Error Categories
///
/// Errors are categorized into two main types:
///
/// ## Retryable Errors
/// These errors indicate temporary conditions that may succeed on retry:
/// - `CommandPending` - Command acknowledged but not yet complete
/// - `CommandBufferFull` - Camera's command buffer is full (always retry)
/// - `NoSocket` - The addressed command socket is no longer available
/// - `RuntimeQueueFull` - The local admission queue is full
/// - `TransportBusy` - The blocking facade is already borrowing the transport
/// - `TransportError` - One transport operation failed while the session remains live
/// - `Timeout` - General timeout condition
///
/// Use [`Error::is_retryable()`] to check if an error can be retried, and
/// [`Error::suggested_retry_delay()`] to get the recommended delay before retrying.
///
/// ## Non-Retryable Errors
/// These errors indicate permanent failures or invalid operations:
/// - `SyntaxError` - Invalid VISCA command format
/// - `CommandNotExecutable` - Command invalid in current state
/// - `InvalidParameter` - Parameter value is invalid
/// - `FeatureNotSupported` - Camera model doesn't support this feature
/// - `InvalidPreset` - Requested preset is outside the profile's supported range
///
/// ## Terminal Session Failures
/// A third category ends the session outright: the peer closed the connection,
/// the byte-stream position became unknowable, or the application shut the
/// runtime down. A fatal receive closure is normalized to
/// [`Error::ConnectionClosed`] (with the transport cause retained in its
/// reason); [`Error::StreamPoisoned`] is reserved for an unknowable stream
/// framing or write position. Use [`Error::requires_new_session()`] to tell
/// transport death from deliberate shutdown. A per-request correlation failure
/// for an unsequenced command on a raw-VISCA envelope is different:
/// [`Error::UnsequencedCommandUnconfirmed`] has
/// [`ErrorKind::Unconfirmed`] and does not by itself make the session unusable.
///
/// # VISCA Error Codes
///
/// The VISCA protocol defines specific error codes that are mapped to Error variants:
/// - `0x01` → `MessageLengthError` - Message length incorrect
/// - `0x02` → `SyntaxError` - Command format invalid
/// - `0x03` → `CommandBufferFull` - Camera busy (always retryable)
/// - `0x04` → `CommandCanceled` - Command was canceled
/// - `0x05` → `NoSocket` - No socket available
/// - `0x41` → `CommandNotExecutable` - Command invalid in current state
///
/// Use [`Error::from_code()`] to convert VISCA error codes to Error variants.
///
/// # Example
///
/// ```rust
/// use grafton_visca::Error;
/// use std::time::Duration;
///
/// fn handle_camera_error(error: Error) -> Result<(), Error> {
///     if error.is_retryable() {
///         if let Some(delay) = error.suggested_retry_delay() {
///             println!("Retrying after {:?}", delay);
///             std::thread::sleep(delay);
///             // Retry the operation...
///         }
///     } else {
///         // Handle permanent error
///         return Err(error);
///     }
///     Ok(())
/// }
/// ```
#[derive(ThisError, Debug, Clone)]
#[non_exhaustive]
pub enum Error {
    /// Failed to establish connection to the camera.
    #[error("Connection failed to {addr}: {source}")]
    ConnectionFailed {
        /// The address that failed to connect.
        addr: Cow<'static, str>,
        /// The underlying IO error.
        source: Arc<io::Error>,
    },

    /// Connection to the camera was closed.
    ///
    /// Owners normalize a fatal receive-side closure to this variant and keep
    /// the underlying transport cause in `reason`. A stream framing or write
    /// failure whose byte position is unknowable is reported as
    /// [`Self::StreamPoisoned`] instead.
    #[error("Connection closed{}", reason.as_ref().map(|r| format!(": {r}")).unwrap_or_default())]
    ConnectionClosed {
        /// Optional reason for the connection closure.
        reason: Option<Cow<'static, str>>,
    },

    /// Command has been acknowledged but is still pending completion.
    /// This is returned when an ACK is received, indicating the command
    /// was queued but not yet executed.
    #[error("Command acknowledged and pending completion")]
    CommandPending,

    /// Response from camera doesn't match the expected format.
    #[error("Invalid response: expected {expected}, got {actual:?}")]
    InvalidResponse {
        /// Description of expected response.
        expected: Cow<'static, str>,
        /// Actual bytes received.
        actual: Vec<u8>,
    },

    /// Camera model doesn't support the requested feature.
    #[error("Feature '{feature}' not supported by this camera model")]
    FeatureNotSupported {
        /// Name of the unsupported feature.
        feature: &'static str,
    },

    /// Underlying IO error from network operations.
    #[error("IO error: {0}")]
    Io(Arc<io::Error>),

    /// VISCA protocol syntax error (0x02): Command format is incorrect or parameters are illegal.
    #[error("Syntax error in VISCA command")]
    SyntaxError,

    /// VISCA protocol command buffer full error (0x03): Two sockets are already in use.
    #[error("Command buffer is full")]
    CommandBufferFull,

    /// VISCA protocol command canceled (0x04): Command was canceled in the specified socket.
    #[error("Command was canceled")]
    CommandCanceled,

    /// VISCA protocol no socket error (0x05): No command is executing in the specified socket.
    #[error("No socket available")]
    NoSocket,

    /// VISCA protocol command not executable (0x41): Command cannot be executed due to current conditions.
    #[error("Command is not executable")]
    CommandNotExecutable,

    /// Response data doesn't conform to expected VISCA protocol format.
    #[error("Invalid response format")]
    InvalidResponseFormat,

    /// Response has an unexpected number of bytes.
    ///
    /// This error includes diagnostic context to help identify camera compatibility issues:
    /// - Expected byte count for this inquiry type
    /// - Actual byte count received
    /// - Hex dump of the payload (truncated if too long)
    #[error(
        "Invalid response length: expected {expected} bytes, got {actual} (payload: {payload_hex})"
    )]
    InvalidResponseLength {
        /// Expected number of bytes for this response type.
        expected: usize,
        /// Actual number of bytes received.
        actual: usize,
        /// Hex representation of the actual payload (truncated if >32 bytes).
        payload_hex: Box<str>,
    },

    /// Response type doesn't match what the command should return.
    #[error("Unexpected response type")]
    UnexpectedResponseType,

    /// Received an unknown error code from the camera.
    #[error("Unknown error code: {0:#02X}")]
    Unknown(u8),

    /// Invalid request to socket manager.
    #[error("Invalid request: {0}")]
    InvalidRequest(Cow<'static, str>),

    /// Message length error (0x01): Message length is incorrect.
    #[error("Message length error")]
    MessageLengthError,

    /// Failed to parse response data.
    #[error("Parse error: {0}")]
    ParseError(Cow<'static, str>),

    /// One transport operation failed while the session remains usable.
    ///
    /// This is a retryable per-request failure with
    /// [`ErrorKind::Transport`]. The receive side remains authoritative for
    /// session death; use [`Self::requires_new_session()`] rather than treating
    /// this error as a reconnect signal.
    #[error("Transport error: {0}")]
    TransportError(Cow<'static, str>),

    /// Invalid parameter provided to a command.
    #[error("Invalid parameter '{parameter}': {reason} (value: {value})")]
    InvalidParameter {
        /// The parameter name that was invalid.
        parameter: &'static str,
        /// The value that was provided.
        value: Cow<'static, str>,
        /// The reason why it's invalid.
        reason: Cow<'static, str>,
    },

    /// Buffer provided is too small for encoding.
    #[error("Buffer too small: required {required} bytes, but only {actual} available")]
    BufferTooSmall {
        /// Required buffer size.
        required: usize,
        /// Actual buffer size provided.
        actual: usize,
    },

    /// Invalid preset number for the camera model.
    #[error("Invalid preset {preset}: must be <= {max}")]
    InvalidPreset {
        /// The requested preset number.
        preset: u8,
        /// Maximum allowed preset for this camera.
        max: u8,
    },

    /// Parameter value is out of the acceptable range.
    #[error("Parameter out of range: {parameter} = {value} (valid range: {min}..{max})")]
    ParameterOutOfRange {
        /// Name of the parameter.
        parameter: &'static str,
        /// Value that was provided.
        value: i32,
        /// Minimum valid value.
        min: i32,
        /// Maximum valid value.
        max: i32,
    },

    /// Operation exceeded timeout without response.
    #[error("Operation timed out")]
    Timeout,

    /// Maximum retry attempts exceeded.
    #[error("Maximum retries exceeded")]
    MaxRetriesExceeded,

    /// Operation is not supported by this implementation.
    #[error("Operation not supported")]
    NotSupported,

    /// Operation cannot be performed in current state.
    #[error("Invalid state: {0}")]
    InvalidState(Cow<'static, str>),

    /// The blocking owner cannot enter this turn without violating exclusive
    /// ownership.
    ///
    /// This has two meanings: a blocking call re-entered an owner turn already
    /// in progress, or a newly submitted operation-handle request could not win
    /// its immediate first-dispatch boundary. The latter includes socket
    /// capacity and an earlier normative scheduler winner. It is never a
    /// peer-disconnect verdict and is not emitted by the async facade.
    #[error("Transport is busy with another operation")]
    TransportBusy,

    /// Runtime has been shutdown.
    #[error("Runtime has been shutdown")]
    RuntimeShutdown,

    /// Cancellation intent was recorded after transmission, but the engine could
    /// not prove either original completion or protocol cancellation before the
    /// correlation quarantine expired.
    #[error("Cancellation could not be confirmed")]
    CancellationUnconfirmed,

    /// An unsequenced command on a raw-VISCA envelope was successfully sent,
    /// but its ACK or completion outcome became unknowable — a lost
    /// ACK/completion datagram, an expired cancellation-ambiguity window, or a
    /// spent retry budget while an attempt was in `Sending`, `AwaitingAck`,
    /// `AwaitingCompletion`, or `Executing`.
    ///
    /// By default this is a **per-request** outcome the session survives (issue
    /// #671): the engine fails only this command and quarantines its correlation
    /// slot — its owned socket, or its place as the sole unacknowledged raw
    /// command — until the ambiguity deadline, so a late reply cannot bind to a
    /// later command. It is never replayed automatically, because an
    /// unsequenced raw-VISCA command may already have reached the camera, and it
    /// is therefore *not* proof of session death:
    /// [`Self::requires_new_session`] is `false`. Reconcile the affected camera
    /// state before deciding whether to resubmit. The strict
    /// [`SessionConfig::with_strict_unconfirmed_poison`] opt-in instead poisons
    /// the whole session, which is surfaced as [`Self::StreamPoisoned`].
    ///
    /// [`SessionConfig::with_strict_unconfirmed_poison`]: crate::SessionConfig::with_strict_unconfirmed_poison
    #[error("Unsequenced command outcome could not be confirmed")]
    UnsequencedCommandUnconfirmed,

    /// A private runtime identity space was exhausted without a safe non-aliasing value.
    #[error("Runtime identity space exhausted")]
    RuntimeIdentityExhausted,

    /// Runtime command queue is at capacity.
    ///
    /// This error indicates that the runtime's pending command queue has reached
    /// its maximum configured depth and cannot accept new commands. This is a
    /// retryable error - callers should back off and retry after a delay.
    ///
    /// The admission capacity is configurable via [`SessionConfig::admission_capacity`].
    ///
    /// [`SessionConfig::admission_capacity`]: crate::SessionConfig::admission_capacity
    #[error("Runtime queue full: at capacity ({capacity} pending commands)")]
    RuntimeQueueFull {
        /// The maximum queue capacity that was reached.
        capacity: usize,
    },

    /// A stream transport's framing or write position became unknowable.
    ///
    /// This error indicates that a stream-based transport (TCP, Serial) lost
    /// framing certainty, for example after a failed or timed-out write that
    /// may have partially reached the wire, or after an unrecoverable framer
    /// overflow. A fatal receive closure is not poison: owners normalize that
    /// case to [`Self::ConnectionClosed`] and retain the receive cause there.
    /// Subsequent commands could otherwise be concatenated onto an incomplete
    /// prior frame, so the stream cannot safely be reused.
    ///
    /// This is a **non-retryable** error that requires establishing a new connection.
    /// All pending commands will receive this error when the transport is poisoned.
    ///
    /// # Recovery
    ///
    /// Create a new transport connection, re-query/reconcile device state, and
    /// only then deliberately resubmit work whose desired effect is still
    /// needed. Never blindly replay a command whose completion is uncertain:
    /// an unsequenced command on a raw-VISCA envelope may already have reached
    /// the camera before its outcome was lost, and the replacement session
    /// cannot prove otherwise.
    #[error("Stream transport poisoned: {reason}")]
    StreamPoisoned {
        /// Description of why the transport was poisoned.
        reason: Cow<'static, str>,
    },

    /// No decoder found for the specified inquiry kind.
    ///
    /// This error indicates that none of the domain-specific decoders
    /// could handle the given `InquiryKind`. This typically means:
    /// - A new `InquiryKind` was added but no decoder was implemented
    /// - The camera returned an unexpected response format
    /// - A decoder is missing for a specific profile's inquiry needs
    #[error("No decoder found for {inquiry_kind:?} (payload: {payload_hex})")]
    DecoderNotFound {
        /// The inquiry kind that no decoder could handle.
        inquiry_kind: crate::command::inquiry_structs::InquiryKind,
        /// Hex representation of the payload for debugging.
        payload_hex: Box<str>,
    },

    /// Invalid camera ID provided.
    #[error("Invalid camera ID {id}: must be 1-7 for individual cameras or 8 for broadcast")]
    InvalidCameraId {
        /// The invalid camera ID that was provided.
        id: u8,
    },

    /// Response exceeds maximum allowed size.
    #[error("Response too large: exceeds maximum of {max_size} bytes")]
    ResponseTooLarge {
        /// Maximum allowed size.
        max_size: usize,
    },

    /// Inquiry not cancelable: `*_with_id` APIs are for commands only.
    ///
    /// Inquiries are not cancelable because they complete immediately without
    /// occupying a VISCA socket. Use `send_inquiry` or `send_inquiry_typed` instead.
    #[error("Inquiries cannot be canceled: use send_inquiry instead of *_with_id APIs")]
    InquiryNotCancelable,

    /// Invalid network address format.
    #[error("Invalid address: {reason}")]
    InvalidAddress {
        /// Reason why the address is invalid.
        reason: Cow<'static, str>,
    },

    /// Selected standard transport is unsupported for the selected built-in profile.
    #[error("Profile {profile} does not support {transport} transport")]
    UnsupportedTransport {
        /// Built-in profile that rejected the transport.
        profile: crate::camera::profiles::ProfileId,
        /// Selected transport kind.
        transport: crate::camera::TransportKind,
    },

    /// Runtime is required for async operations but was not provided.
    #[error("No runtime configured for async operations")]
    MissingRuntime,

    /// Error with additional context information.
    /// Wraps another error while preserving its retry intelligence and adding human-readable context.
    #[error("{context}: {source}")]
    WithContext {
        /// Human-readable context describing what operation failed.
        context: Cow<'static, str>,
        /// The underlying error that occurred.
        source: Box<Error>,
    },
}

impl Error {
    /// Get the kind of this error for categorized handling.
    ///
    /// # Examples
    /// ```rust
    /// use grafton_visca::{Error, ErrorKind};
    ///
    /// let error = Error::Timeout;
    /// assert_eq!(error.kind(), ErrorKind::Timeout);
    /// ```
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        match self {
            // Timeout: transient timing failures
            Self::Timeout | Self::MaxRetriesExceeded => ErrorKind::Timeout,

            // Cancelled: explicit cancellation
            Self::CommandCanceled => ErrorKind::Cancelled,

            // BufferFull: transient capacity exhaustion
            Self::CommandBufferFull | Self::RuntimeQueueFull { .. } | Self::NoSocket => {
                ErrorKind::BufferFull
            }

            // NotExecutable: command invalid in current state
            Self::CommandNotExecutable | Self::InvalidState(..) => ErrorKind::NotExecutable,

            // IoClosed: connection/transport no longer usable
            Self::ConnectionClosed { .. } | Self::RuntimeShutdown | Self::StreamPoisoned { .. } => {
                ErrorKind::IoClosed
            }

            // Transport: one failed operation on a live session.
            Self::TransportError(..) => ErrorKind::Transport,

            // Unconfirmed: one transmitted operation has an unknowable result
            Self::CancellationUnconfirmed | Self::UnsequencedCommandUnconfirmed => {
                ErrorKind::Unconfirmed
            }

            // IoRefused: connection attempt rejected
            Self::ConnectionFailed { .. } => ErrorKind::IoRefused,

            // Protocol: malformed or unexpected wire data
            Self::InvalidResponseFormat
            | Self::InvalidResponseLength { .. }
            | Self::UnexpectedResponseType
            | Self::ParseError { .. }
            | Self::MessageLengthError
            | Self::InvalidResponse { .. }
            | Self::Unknown(..)
            | Self::DecoderNotFound { .. }
            | Self::ResponseTooLarge { .. } => ErrorKind::Protocol,

            // Unsupported: feature/capability not available
            Self::FeatureNotSupported { .. } | Self::NotSupported | Self::MissingRuntime => {
                ErrorKind::Unsupported
            }

            // InvalidParameter: caller-supplied value is invalid
            Self::InvalidParameter { .. }
            | Self::InvalidPreset { .. }
            | Self::ParameterOutOfRange { .. }
            | Self::SyntaxError
            | Self::InvalidRequest(..)
            | Self::BufferTooSmall { .. }
            | Self::InvalidCameraId { .. }
            | Self::InquiryNotCancelable { .. }
            | Self::InvalidAddress { .. } => ErrorKind::InvalidParameter,
            Self::UnsupportedTransport { .. } => ErrorKind::Unsupported,

            // Busy: transient contention
            Self::TransportBusy | Self::CommandPending => ErrorKind::Busy,

            // Other: truly uncategorizable
            Self::RuntimeIdentityExhausted => ErrorKind::Other,

            // Delegated: unwrap context wrapper
            Self::WithContext { source, .. } => source.kind(),

            // Io: inspect inner io::ErrorKind
            Self::Io(arc) => match arc.kind() {
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => ErrorKind::Timeout,
                io::ErrorKind::ConnectionRefused => ErrorKind::IoRefused,
                io::ErrorKind::ConnectionReset
                | io::ErrorKind::ConnectionAborted
                | io::ErrorKind::BrokenPipe
                | io::ErrorKind::UnexpectedEof
                | io::ErrorKind::NotConnected => ErrorKind::IoClosed,
                _ => ErrorKind::Other,
            },
        }
    }

    /// Reports whether the session that produced this error is permanently
    /// unusable, so recovery must construct a replacement session.
    ///
    /// Transport close, explicit shutdown, and poison are given distinct
    /// terminal errors, but they all share [`ErrorKind::IoClosed`]. Matching on
    /// the kind therefore cannot separate "the camera dropped the connection"
    /// from "this application asked the runtime to stop". This predicate draws
    /// exactly that line without exposing implementation details:
    ///
    /// - `true` — the transport died underneath the session: the peer closed
    ///   the connection ([`Error::ConnectionClosed`]) or the byte-stream
    ///   position became unknowable ([`Error::StreamPoisoned`]). A poisoned
    ///   session is terminal and is never revived; every retained and
    ///   subsequently attempted operation keeps reporting its exact terminal
    ///   session error. Fatal receive closure is always normalized to
    ///   `ConnectionClosed`, with the underlying cause in its reason. An
    ///   unconfirmed unsequenced raw-VISCA command is not in this set by default: its
    ///   [`Error::UnsequencedCommandUnconfirmed`] result is per-request and
    ///   leaves the session running.
    /// - `false` — the condition does not prove the session is unusable. A
    ///   deliberate [`Error::RuntimeShutdown`] is the important case: the
    ///   application ended that session on purpose and must not treat it as a
    ///   field disconnect to reconnect around. Ordinary per-request failures
    ///   (timeouts, busy states, protocol and parameter errors) are also
    ///   `false`, and so is a raw [`Error::Io`] failure, because a datagram
    ///   write failure is isolated to its own transmission and a stream failure
    ///   is reported to the caller as [`Error::StreamPoisoned`].
    ///
    /// `true` is positive proof that the session is finished. `false` only
    /// means this error alone does not establish it.
    ///
    /// # Recovery
    ///
    /// Keep the reusable [`SessionConfig`], build a fresh transport, and open a
    /// new session; the old session, camera views, operation handles, and
    /// subscriptions cannot be rebound to it. The new session's state cache
    /// starts `Unknown` and nothing is resubmitted automatically, so re-query
    /// supported camera state before applying desired state. Do not blindly
    /// replay a command whose completion is uncertain: it may have reached the
    /// camera before the connection failed or unsequenced raw-VISCA
    /// correlation was poisoned.
    ///
    /// [`SessionConfig`]: crate::SessionConfig
    ///
    /// # Example
    ///
    /// ```rust
    /// use grafton_visca::Error;
    ///
    /// let dropped = Error::ConnectionClosed { reason: None };
    /// assert!(dropped.requires_new_session());
    ///
    /// // A deliberate shutdown is not a field disconnect.
    /// assert!(!Error::RuntimeShutdown.requires_new_session());
    ///
    /// // Both share one kind, so kind() alone cannot separate them.
    /// assert_eq!(dropped.kind(), Error::RuntimeShutdown.kind());
    /// ```
    #[must_use]
    pub fn requires_new_session(&self) -> bool {
        match self {
            // Terminal session death that the application did not ask for. The
            // engine and both owners report exactly these variants when a
            // session stops being usable because of the transport.
            Self::ConnectionClosed { .. } | Self::StreamPoisoned { .. } => true,

            // Deliberate shutdown. The session is over because the application
            // ended it, so reconnecting is a policy decision, not a repair.
            Self::RuntimeShutdown => false,

            // Delegated: an added context string never changes the condition.
            Self::WithContext { source, .. } => source.requires_new_session(),

            // Everything else is either a per-request outcome or a condition
            // the session survives. `Io` and `TransportError` stay here on
            // purpose: a datagram request-write failure is isolated to its own
            // transmission, and a stream failure reaches the caller as
            // `StreamPoisoned` from the owner, so the raw transport error is
            // never the proof of session death. `UnsequencedCommandUnconfirmed`
            // is also here (issue #671): by default it fails one raw command
            // while the session keeps running, and the strict opt-in reports
            // session death separately as `StreamPoisoned`. A live session must
            // never hand out a replacement-session verdict.
            Self::ConnectionFailed { .. }
            | Self::CommandPending
            | Self::InvalidResponse { .. }
            | Self::FeatureNotSupported { .. }
            | Self::Io(..)
            | Self::SyntaxError
            | Self::CommandBufferFull
            | Self::CommandCanceled
            | Self::NoSocket
            | Self::CommandNotExecutable
            | Self::InvalidResponseFormat
            | Self::InvalidResponseLength { .. }
            | Self::UnexpectedResponseType
            | Self::Unknown(..)
            | Self::InvalidRequest(..)
            | Self::MessageLengthError
            | Self::ParseError(..)
            | Self::TransportError(..)
            | Self::InvalidParameter { .. }
            | Self::BufferTooSmall { .. }
            | Self::InvalidPreset { .. }
            | Self::ParameterOutOfRange { .. }
            | Self::Timeout
            | Self::MaxRetriesExceeded
            | Self::NotSupported
            | Self::InvalidState(..)
            | Self::TransportBusy
            | Self::CancellationUnconfirmed
            | Self::UnsequencedCommandUnconfirmed
            | Self::RuntimeIdentityExhausted
            | Self::RuntimeQueueFull { .. }
            | Self::DecoderNotFound { .. }
            | Self::InvalidCameraId { .. }
            | Self::ResponseTooLarge { .. }
            | Self::InquiryNotCancelable
            | Self::InvalidAddress { .. }
            | Self::UnsupportedTransport { .. }
            | Self::MissingRuntime => false,
        }
    }

    /// Create an `Error` from a VISCA error response code.
    ///
    /// ## Error Code Mapping
    ///
    /// - `0x01`: Message Length Error - message length incorrect
    /// - `0x02`: Syntax Error - command format invalid
    /// - `0x03`: Command Buffer Full - camera busy, always retry later
    /// - `0x04`: Command Canceled - command was canceled
    /// - `0x05`: No Socket - no socket available
    /// - `0x41`: Command Not Executable - command invalid in current state
    ///
    /// Note: 0x41 is context-dependent. Some cameras (e.g., FR7) use it to indicate
    /// "still settling after preset, retry" while others mean "invalid command".
    /// The runtime determines retry policy based on camera profile and command context.
    #[must_use]
    pub const fn from_code(code: u8) -> Self {
        match code {
            0x01 => Self::MessageLengthError,
            0x02 => Self::SyntaxError,
            0x03 => Self::CommandBufferFull, // Always retryable
            0x04 => Self::CommandCanceled,
            0x05 => Self::NoSocket,
            0x41 => Self::CommandNotExecutable, // Context-dependent retryability
            _ => Self::Unknown(code),
        }
    }

    /// Check if this error is potentially retryable.
    ///
    /// Returns `true` for errors that represent temporary conditions
    /// that may succeed if the operation is retried. This includes:
    /// - Camera capacity states (`CommandBufferFull`, `NoSocket`)
    /// - Queue capacity (`RuntimeQueueFull`)
    /// - Pending operations (`CommandPending`, `TransportBusy`)
    /// - Isolated live-session transport failures (`TransportError`)
    /// - Timeout conditions (`Timeout` and timed-out I/O)
    ///
    /// # Example
    ///
    /// ```rust
    /// use grafton_visca::Error;
    ///
    /// let error = Error::CommandBufferFull;
    /// if error.is_retryable() {
    ///     println!("This error can be retried");
    /// }
    /// ```
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::WithContext { source, .. } => source.is_retryable(),
            // MaxRetriesExceeded maps to Timeout (retryable kind) but must not
            // itself be retried — doing so would cause infinite retry loops.
            Self::MaxRetriesExceeded => false,
            _ => matches!(
                self.kind(),
                ErrorKind::Timeout | ErrorKind::BufferFull | ErrorKind::Busy | ErrorKind::Transport
            ),
        }
    }

    /// Get a suggested retry delay for retryable errors.
    ///
    /// Returns `Some(Duration)` with a recommended delay before retrying
    /// the operation, or `None` if the error is not retryable.
    /// Every error for which [`Self::is_retryable()`] returns `true` has a
    /// suggested delay.
    ///
    /// The suggested delays are based on typical camera response times:
    /// - `CommandPending`: 50ms (command acknowledged, waiting for completion)
    /// - `TransportBusy`: 50ms (blocking transport borrow is occupied)
    /// - `CommandBufferFull`: 200ms (wait for buffer space)
    /// - `NoSocket`: 200ms (wait for camera socket state to advance)
    /// - `TransportError`: 50ms (retry one isolated live-session operation)
    /// - `Timeout`: 2s (general timeout, allow more time)
    ///
    /// # Example
    ///
    /// ```rust
    /// use grafton_visca::Error;
    /// use std::time::Duration;
    ///
    /// let error = Error::CommandBufferFull;
    /// if let Some(delay) = error.suggested_retry_delay() {
    ///     assert_eq!(delay, Duration::from_millis(200));
    ///     std::thread::sleep(delay);
    ///     // Retry the operation...
    /// }
    /// ```
    #[must_use]
    pub fn suggested_retry_delay(&self) -> Option<Duration> {
        match self {
            Self::CommandPending => Some(Duration::from_millis(50)),
            Self::TransportBusy => Some(Duration::from_millis(50)),
            Self::TransportError(..) => Some(Duration::from_millis(50)),
            Self::CommandBufferFull | Self::RuntimeQueueFull { .. } | Self::NoSocket => {
                Some(Duration::from_millis(200))
            }
            Self::Timeout => Some(Duration::from_secs(2)),
            Self::MaxRetriesExceeded => None,
            Self::WithContext { source, .. } => source.suggested_retry_delay(),
            // Keep the fallback aligned with `is_retryable()`'s `ErrorKind`
            // classification. This includes I/O timeout spellings and gives
            // newly classified retryable errors a safe default delay.
            _ => match self.kind() {
                ErrorKind::Timeout => Some(Duration::from_secs(2)),
                ErrorKind::BufferFull => Some(Duration::from_millis(200)),
                ErrorKind::Busy | ErrorKind::Transport => Some(Duration::from_millis(50)),
                _ => None,
            },
        }
    }

    /// Add operation context to this error.
    ///
    /// Wraps the error with additional context information while preserving
    /// retry intelligence (e.g., `is_retryable()`, `suggested_retry_delay()`).
    ///
    /// This is useful for providing more detailed error messages that explain
    /// what operation was being performed when the error occurred.
    ///
    /// # Example
    ///
    /// ```rust
    /// use grafton_visca::Error;
    ///
    /// let error = Error::CommandBufferFull;
    /// let contextual_error = error.with_context("Failed to recall preset 5");
    ///
    /// // Original error properties are preserved
    /// assert!(contextual_error.is_retryable());
    /// assert!(contextual_error.suggested_retry_delay().is_some());
    ///
    /// // But the error message now includes context
    /// assert_eq!(
    ///     contextual_error.to_string(),
    ///     "Failed to recall preset 5: Command buffer is full"
    /// );
    /// ```
    #[must_use]
    pub fn with_context(self, context: impl Into<Cow<'static, str>>) -> Self {
        Self::WithContext {
            context: context.into(),
            source: Box::new(self),
        }
    }

    /// Add operation context to this error using a `Display` type.
    ///
    /// Similar to [`Error::with_context`], but accepts any type that implements `Display`.
    ///
    /// This is useful for providing formatted context messages.
    ///
    /// # Example
    ///
    /// ```rust
    /// use grafton_visca::Error;
    ///
    /// let preset_id = 5;
    /// let error = Error::CommandBufferFull;
    /// let contextual_error = error.context(format!("Failed to recall preset {}", preset_id));
    ///
    /// assert_eq!(
    ///     contextual_error.to_string(),
    ///     "Failed to recall preset 5: Command buffer is full"
    /// );
    /// ```
    #[must_use]
    pub fn context<D: std::fmt::Display>(self, context: D) -> Self {
        self.with_context(context.to_string())
    }

    /// Create an `InvalidResponseLength` error with full diagnostic context.
    ///
    /// This constructor captures the expected length, actual length, and a hex dump
    /// of the received payload to help diagnose camera compatibility issues.
    ///
    /// # Example
    ///
    /// ```rust
    /// use grafton_visca::Error;
    ///
    /// let payload = &[0x00, 0x52];
    /// let error = Error::invalid_response_length(7, payload);
    /// assert_eq!(
    ///     error.to_string(),
    ///     "Invalid response length: expected 7 bytes, got 2 (payload: 00 52)"
    /// );
    /// ```
    #[must_use]
    pub fn invalid_response_length(expected: usize, payload: &[u8]) -> Self {
        Self::InvalidResponseLength {
            expected,
            actual: payload.len(),
            payload_hex: format_payload_hex(payload),
        }
    }
}

/// Format a byte slice as a hex string, truncating if too long.
///
/// Payloads longer than 32 bytes are truncated with "..." and a byte count suffix.
pub(crate) fn format_payload_hex(payload: &[u8]) -> Box<str> {
    const MAX_DISPLAY_BYTES: usize = 32;

    if payload.is_empty() {
        return "(empty)".into();
    }

    if payload.len() <= MAX_DISPLAY_BYTES {
        payload
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
            .into_boxed_str()
    } else {
        let truncated: String = payload[..MAX_DISPLAY_BYTES]
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ");
        format!("{truncated}... ({} bytes total)", payload.len()).into_boxed_str()
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::Io(Arc::new(err))
    }
}

impl From<nom::Err<nom::error::Error<&[u8]>>> for Error {
    fn from(err: nom::Err<nom::error::Error<&[u8]>>) -> Self {
        Self::ParseError(Cow::Owned(err.to_string()))
    }
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unreachable!("Infallible error should never occur")
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;

    #[test]
    fn test_visca_error_from_code() {
        assert!(matches!(Error::from_code(0x01), Error::MessageLengthError));
        assert!(matches!(Error::from_code(0x02), Error::SyntaxError));
        assert!(matches!(Error::from_code(0x03), Error::CommandBufferFull));
        assert!(matches!(Error::from_code(0x04), Error::CommandCanceled));
        assert!(matches!(Error::from_code(0x05), Error::NoSocket));
        assert!(matches!(
            Error::from_code(0x41),
            Error::CommandNotExecutable
        ));
        assert!(matches!(Error::from_code(0xFF), Error::Unknown(0xFF)));
    }

    #[test]
    fn test_error_code_mapping_table() {
        // Table-driven test for all known VISCA error codes
        // This ensures consistent mapping across all layers
        let cases = [
            (0x01, "MessageLengthError"),
            (0x02, "SyntaxError"),
            (0x03, "CommandBufferFull"),
            (0x04, "CommandCanceled"),
            (0x05, "NoSocket"),
            // 0x41 is "Command Not Executable" per VISCA spec - command invalid in current state
            (0x41, "CommandNotExecutable"),
        ];

        for (byte, expected_variant) in cases {
            let error = Error::from_code(byte);
            let variant_name = match error {
                Error::MessageLengthError => "MessageLengthError",
                Error::SyntaxError => "SyntaxError",
                Error::CommandBufferFull => "CommandBufferFull",
                Error::CommandCanceled => "CommandCanceled",
                Error::NoSocket => "NoSocket",
                Error::CommandNotExecutable => "CommandNotExecutable",
                _ => "Unknown",
            };
            assert_eq!(
                variant_name, expected_variant,
                "Byte {:#04x} should map to {}",
                byte, expected_variant
            );
        }
    }

    #[test]
    fn test_unknown_error_code_maps_to_unknown() {
        // Test that unrecognized error codes map to Unknown variant
        let unknown_codes = [0x00, 0x06, 0x10, 0x20, 0x30, 0x40, 0x42, VISCA_TERMINATOR];

        for code in unknown_codes {
            let error = Error::from_code(code);
            assert!(
                matches!(error, Error::Unknown(c) if c == code),
                "Code {:#04x} should map to Unknown({})",
                code,
                code
            );
        }
    }

    #[test]
    fn test_visca_error_display() {
        assert_eq!(
            Error::SyntaxError.to_string(),
            "Syntax error in VISCA command"
        );
        assert_eq!(
            Error::CommandBufferFull.to_string(),
            "Command buffer is full"
        );
        assert_eq!(Error::CommandCanceled.to_string(), "Command was canceled");
        assert_eq!(Error::NoSocket.to_string(), "No socket available");
        assert_eq!(
            Error::CommandNotExecutable.to_string(),
            "Command is not executable"
        );
        assert_eq!(Error::Unknown(0x99).to_string(), "Unknown error code: 0x99");
        assert_eq!(
            Error::InvalidParameter {
                parameter: "test",
                value: Cow::Borrowed("invalid"),
                reason: Cow::Borrowed("test reason"),
            }
            .to_string(),
            "Invalid parameter 'test': test reason (value: invalid)"
        );
        assert_eq!(Error::Timeout.to_string(), "Operation timed out");
    }

    #[test]
    fn test_visca_error_from_io_error() {
        let io_err = io::Error::other("test error");
        let visca_err = Error::from(io_err);
        assert!(matches!(visca_err, Error::Io(_)));
    }

    #[test]
    fn test_visca_error_from_nom_error() {
        use nom::error::{Error as NomError, ErrorKind};

        let nom_err = nom::Err::Error(NomError::new(&b"test"[..], ErrorKind::Tag));
        let visca_err = Error::from(nom_err);
        assert!(matches!(visca_err, Error::ParseError(_)));
    }

    #[test]
    fn test_is_retryable() {
        assert!(Error::CommandBufferFull.is_retryable());
        assert!(Error::Timeout.is_retryable());
        assert!(Error::RuntimeQueueFull { capacity: 8 }.is_retryable());
        assert!(Error::TransportError(Cow::Borrowed("datagram send failed")).is_retryable());

        // Issue #501: TransportBusy is transient and should be retryable
        assert!(Error::TransportBusy.is_retryable());
        // Issue #501: NoSocket is transient capacity and should be retryable
        assert!(Error::NoSocket.is_retryable());
        // Issue #501: CommandPending is now Busy via kind(), no special-case needed
        assert!(Error::CommandPending.is_retryable());
        // Issue #501: MaxRetriesExceeded must NOT be retryable (prevents infinite loops)
        assert!(!Error::MaxRetriesExceeded.is_retryable());
        assert!(!Error::UnsequencedCommandUnconfirmed.is_retryable());

        assert!(!Error::SyntaxError.is_retryable());
        assert!(!Error::CommandNotExecutable.is_retryable());
        assert!(!Error::InvalidParameter {
            parameter: "test",
            value: Cow::Borrowed("invalid"),
            reason: Cow::Borrowed("test reason"),
        }
        .is_retryable());
        assert!(!Error::InvalidPreset { preset: 9, max: 8 }.is_retryable());
    }

    #[test]
    fn test_suggested_retry_delay() {
        assert_eq!(
            Error::CommandBufferFull.suggested_retry_delay(),
            Some(Duration::from_millis(200))
        );
        assert_eq!(
            Error::CommandPending.suggested_retry_delay(),
            Some(Duration::from_millis(50))
        );
        assert_eq!(
            Error::Timeout.suggested_retry_delay(),
            Some(Duration::from_secs(2))
        );
        assert_eq!(
            Error::TransportError(Cow::Borrowed("datagram send failed")).suggested_retry_delay(),
            Some(Duration::from_millis(50))
        );

        // Issue #501: TransportBusy → 50ms (extremely transient borrow conflict)
        assert_eq!(
            Error::TransportBusy.suggested_retry_delay(),
            Some(Duration::from_millis(50))
        );
        // Issue #501: NoSocket grouped with CommandBufferFull at 200ms
        assert_eq!(
            Error::NoSocket.suggested_retry_delay(),
            Some(Duration::from_millis(200))
        );
        assert_eq!(
            Error::UnsequencedCommandUnconfirmed.suggested_retry_delay(),
            None
        );

        assert_eq!(Error::SyntaxError.suggested_retry_delay(), None);
        assert_eq!(
            Error::InvalidParameter {
                parameter: "test",
                value: Cow::Borrowed("invalid"),
                reason: Cow::Borrowed("test reason"),
            }
            .suggested_retry_delay(),
            None
        );
    }

    #[test]
    fn retry_helpers_agree_for_public_error_classifications() {
        let cases = [
            (
                "explicit timeout",
                Error::Timeout,
                ErrorKind::Timeout,
                Some(Duration::from_secs(2)),
            ),
            (
                "pending command",
                Error::CommandPending,
                ErrorKind::Busy,
                Some(Duration::from_millis(50)),
            ),
            (
                "transport busy",
                Error::TransportBusy,
                ErrorKind::Busy,
                Some(Duration::from_millis(50)),
            ),
            (
                "isolated datagram transport failure",
                Error::TransportError(Cow::Borrowed("datagram send failed")),
                ErrorKind::Transport,
                Some(Duration::from_millis(50)),
            ),
            (
                "camera command buffer full",
                Error::CommandBufferFull,
                ErrorKind::BufferFull,
                Some(Duration::from_millis(200)),
            ),
            (
                "runtime queue full",
                Error::RuntimeQueueFull { capacity: 64 },
                ErrorKind::BufferFull,
                Some(Duration::from_millis(200)),
            ),
            (
                "camera has no free socket",
                Error::NoSocket,
                ErrorKind::BufferFull,
                Some(Duration::from_millis(200)),
            ),
            (
                "I/O timed out",
                Error::from(io::Error::from(io::ErrorKind::TimedOut)),
                ErrorKind::Timeout,
                Some(Duration::from_secs(2)),
            ),
            (
                "I/O would block",
                Error::from(io::Error::from(io::ErrorKind::WouldBlock)),
                ErrorKind::Timeout,
                Some(Duration::from_secs(2)),
            ),
            (
                "contextual I/O timeout",
                Error::from(io::Error::from(io::ErrorKind::TimedOut))
                    .with_context("read camera reply"),
                ErrorKind::Timeout,
                Some(Duration::from_secs(2)),
            ),
            (
                "retry budget exhausted",
                Error::MaxRetriesExceeded,
                ErrorKind::Timeout,
                None,
            ),
            (
                "I/O connection reset",
                Error::from(io::Error::from(io::ErrorKind::ConnectionReset)),
                ErrorKind::IoClosed,
                None,
            ),
            (
                "I/O connection refused",
                Error::from(io::Error::from(io::ErrorKind::ConnectionRefused)),
                ErrorKind::IoRefused,
                None,
            ),
            (
                "I/O broken pipe",
                Error::from(io::Error::from(io::ErrorKind::BrokenPipe)),
                ErrorKind::IoClosed,
                None,
            ),
            (
                "I/O interrupted",
                Error::from(io::Error::from(io::ErrorKind::Interrupted)),
                ErrorKind::Other,
                None,
            ),
            (
                "terminal closed connection",
                Error::ConnectionClosed { reason: None },
                ErrorKind::IoClosed,
                None,
            ),
            (
                "terminal stream poison",
                Error::StreamPoisoned {
                    reason: Cow::Borrowed("partial write"),
                },
                ErrorKind::IoClosed,
                None,
            ),
            (
                "deliberate runtime shutdown",
                Error::RuntimeShutdown,
                ErrorKind::IoClosed,
                None,
            ),
            (
                "unconfirmed unsequenced raw-VISCA command",
                Error::UnsequencedCommandUnconfirmed,
                ErrorKind::Unconfirmed,
                None,
            ),
            (
                "unconfirmed cancellation",
                Error::CancellationUnconfirmed,
                ErrorKind::Unconfirmed,
                None,
            ),
            (
                "syntax error",
                Error::SyntaxError,
                ErrorKind::InvalidParameter,
                None,
            ),
            (
                "command not executable",
                Error::CommandNotExecutable,
                ErrorKind::NotExecutable,
                None,
            ),
            (
                "invalid preset",
                Error::InvalidPreset { preset: 9, max: 8 },
                ErrorKind::InvalidParameter,
                None,
            ),
            (
                "unsupported feature",
                Error::FeatureNotSupported { feature: "test" },
                ErrorKind::Unsupported,
                None,
            ),
            (
                "invalid parameter",
                Error::InvalidParameter {
                    parameter: "test",
                    value: Cow::Borrowed("invalid"),
                    reason: Cow::Borrowed("test reason"),
                },
                ErrorKind::InvalidParameter,
                None,
            ),
        ];

        for (name, error, expected_kind, expected_delay) in cases {
            assert_eq!(error.kind(), expected_kind, "{name} has the wrong kind");

            let suggested_delay = error.suggested_retry_delay();
            assert_eq!(
                error.is_retryable(),
                suggested_delay.is_some(),
                "{name}: is_retryable() and suggested_retry_delay() disagree"
            );
            assert_eq!(
                suggested_delay, expected_delay,
                "{name} has the wrong suggested retry delay"
            );
        }
    }

    #[test]
    fn test_exhaustive_error_kind_mapping() {
        // Verify specific kind() mappings per issue #501

        // Timeout
        assert_eq!(Error::Timeout.kind(), ErrorKind::Timeout);
        assert_eq!(Error::MaxRetriesExceeded.kind(), ErrorKind::Timeout);

        // Cancelled
        assert_eq!(Error::CommandCanceled.kind(), ErrorKind::Cancelled);

        // BufferFull
        assert_eq!(Error::CommandBufferFull.kind(), ErrorKind::BufferFull);
        assert_eq!(
            Error::RuntimeQueueFull { capacity: 64 }.kind(),
            ErrorKind::BufferFull
        );
        assert_eq!(Error::NoSocket.kind(), ErrorKind::BufferFull);

        // NotExecutable
        assert_eq!(Error::CommandNotExecutable.kind(), ErrorKind::NotExecutable);
        assert_eq!(
            Error::InvalidState(Cow::Borrowed("test")).kind(),
            ErrorKind::NotExecutable
        );

        // IoClosed
        assert_eq!(
            Error::ConnectionClosed { reason: None }.kind(),
            ErrorKind::IoClosed
        );
        assert_eq!(Error::RuntimeShutdown.kind(), ErrorKind::IoClosed);
        // Transport
        assert_eq!(
            Error::TransportError(Cow::Borrowed("test")).kind(),
            ErrorKind::Transport
        );
        // Unconfirmed
        assert_eq!(
            Error::UnsequencedCommandUnconfirmed.kind(),
            ErrorKind::Unconfirmed
        );
        assert_eq!(
            Error::CancellationUnconfirmed.kind(),
            ErrorKind::Unconfirmed
        );

        // Protocol
        assert_eq!(Error::Unknown(0xFF).kind(), ErrorKind::Protocol);

        // Unsupported
        assert_eq!(Error::MissingRuntime.kind(), ErrorKind::Unsupported);

        // InvalidParameter
        assert_eq!(
            Error::InvalidPreset { preset: 9, max: 8 }.kind(),
            ErrorKind::InvalidParameter
        );
        assert_eq!(
            Error::InvalidRequest(Cow::Borrowed("test")).kind(),
            ErrorKind::InvalidParameter
        );
        assert_eq!(
            Error::InvalidAddress {
                reason: Cow::Borrowed("test"),
            }
            .kind(),
            ErrorKind::InvalidParameter
        );

        // Busy
        assert_eq!(Error::TransportBusy.kind(), ErrorKind::Busy);
        assert_eq!(Error::CommandPending.kind(), ErrorKind::Busy);

        // Other
        assert_eq!(Error::RuntimeIdentityExhausted.kind(), ErrorKind::Other);

        // Io: inspect inner io::ErrorKind
        assert_eq!(
            Error::Io(Arc::new(io::Error::new(io::ErrorKind::TimedOut, "test"))).kind(),
            ErrorKind::Timeout
        );
        assert_eq!(
            Error::Io(Arc::new(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                "test"
            )))
            .kind(),
            ErrorKind::IoRefused
        );
        assert_eq!(
            Error::Io(Arc::new(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "test"
            )))
            .kind(),
            ErrorKind::IoClosed
        );
        assert_eq!(
            Error::Io(Arc::new(io::Error::new(io::ErrorKind::BrokenPipe, "test"))).kind(),
            ErrorKind::IoClosed
        );
        assert_eq!(
            Error::Io(Arc::new(io::Error::other("test"))).kind(),
            ErrorKind::Other
        );
    }

    #[test]
    fn requires_new_session_separates_transport_death_from_deliberate_shutdown() {
        // Issue #564: `IoClosed` conflates the three distinct terminal session
        // errors, so the classification must not be derived from the kind.
        let dropped = Error::ConnectionClosed {
            reason: Some(Cow::Borrowed("closed by peer")),
        };
        let poisoned = Error::StreamPoisoned {
            reason: Cow::Borrowed("partial write"),
        };
        let shutdown = Error::RuntimeShutdown;

        assert_eq!(dropped.kind(), ErrorKind::IoClosed);
        assert_eq!(poisoned.kind(), ErrorKind::IoClosed);
        assert_eq!(shutdown.kind(), ErrorKind::IoClosed);

        assert!(dropped.requires_new_session());
        assert!(poisoned.requires_new_session());
        assert!(!shutdown.requires_new_session());
        // Issue #726 gives recoverable correlation ambiguity its own category;
        // it must neither masquerade as a closed connection nor require one.
        assert_eq!(
            Error::UnsequencedCommandUnconfirmed.kind(),
            ErrorKind::Unconfirmed
        );
        assert!(!Error::UnsequencedCommandUnconfirmed.requires_new_session());
    }

    #[test]
    fn requires_new_session_covers_every_unusable_transport_condition() {
        for error in [
            Error::ConnectionClosed { reason: None },
            Error::StreamPoisoned {
                reason: Cow::Borrowed("framing failure"),
            },
        ] {
            assert!(
                error.requires_new_session(),
                "error should require a new session: {error}"
            );
        }
    }

    #[test]
    fn requires_new_session_is_false_for_recoverable_and_deliberate_conditions() {
        for error in [
            Error::RuntimeShutdown,
            Error::Timeout,
            Error::CommandBufferFull,
            Error::CommandPending,
            Error::RuntimeQueueFull { capacity: 8 },
            Error::CommandCanceled,
            Error::CommandNotExecutable,
            Error::SyntaxError,
            Error::NoSocket,
            Error::TransportBusy,
            Error::MaxRetriesExceeded,
            Error::CancellationUnconfirmed,
            // Issue #671: a raw command's default unconfirmed outcome fails the
            // one request while the session keeps running.
            Error::UnsequencedCommandUnconfirmed,
            Error::TransportError(Cow::Borrowed("serial encode failed")),
            Error::ConnectionFailed {
                addr: Cow::Borrowed("192.168.1.100:5678"),
                source: Arc::new(io::Error::other("refused")),
            },
            Error::Io(Arc::new(io::Error::new(io::ErrorKind::BrokenPipe, "pipe"))),
        ] {
            assert!(
                !error.requires_new_session(),
                "error should not require a new session: {error}"
            );
        }
    }

    /// Issue #614: the `0x05` camera answer must never be normalized into a
    /// session-death variant. It is a transient capacity answer from a live
    /// session (issues #501/#566), so any helper that folded it into a
    /// `requires_new_session()` variant would turn a retry into a spurious
    /// reconnect.
    #[test]
    fn no_socket_is_never_a_session_death_condition() {
        let from_camera = Error::from_code(0x05);
        assert!(matches!(from_camera, Error::NoSocket));
        assert!(from_camera.is_retryable());
        assert!(!from_camera.requires_new_session());
        let with_context = from_camera.with_context("cancel command");
        assert!(!with_context.requires_new_session());
    }

    #[test]
    fn requires_new_session_survives_added_context() {
        let error = Error::StreamPoisoned {
            reason: Cow::Borrowed("partial write"),
        }
        .with_context("recall preset 3")
        .with_context("restore show state");
        assert!(error.requires_new_session());

        let deliberate = Error::RuntimeShutdown.with_context("application teardown");
        assert!(!deliberate.requires_new_session());
    }

    #[test]
    fn test_error_implements_clone() {
        // Test that Error implements Clone for various variants
        let error1 = Error::CommandPending;
        let error2 = error1.clone();
        assert!(matches!(error2, Error::CommandPending));

        let error3 = Error::ConnectionFailed {
            addr: Cow::Borrowed("192.168.1.100:5678"),
            source: Arc::new(io::Error::other("test error")),
        };
        let error4 = error3.clone();
        assert!(matches!(error4, Error::ConnectionFailed { .. }));

        let error5 = Error::Io(Arc::new(io::Error::other("test io error")));
        let error6 = error5.clone();
        assert!(matches!(error6, Error::Io(_)));

        let error7 = Error::InvalidParameter {
            parameter: "test",
            value: Cow::Borrowed("value"),
            reason: Cow::Borrowed("reason"),
        };
        let error8 = error7.clone();
        assert!(matches!(
            error8,
            Error::InvalidParameter {
                parameter: "test",
                ..
            }
        ));
    }

    #[test]
    fn test_with_context_preserves_retry_intelligence() {
        // Test that with_context preserves is_retryable
        let error = Error::CommandBufferFull;
        let contextual = error.with_context("Failed to power on camera");
        assert!(contextual.is_retryable());
        assert_eq!(
            contextual.suggested_retry_delay(),
            Some(Duration::from_millis(200))
        );

        // Test with non-retryable error
        let error = Error::SyntaxError;
        let contextual = error.with_context("Failed to send command");
        assert!(!contextual.is_retryable());
        assert_eq!(contextual.suggested_retry_delay(), None);
    }

    #[test]
    fn test_with_context_message_format() {
        let error = Error::CommandBufferFull;
        let contextual = error.with_context("Failed to recall preset 5");
        assert_eq!(
            contextual.to_string(),
            "Failed to recall preset 5: Command buffer is full"
        );
    }

    #[test]
    fn test_context_method() {
        let preset_id = 5;
        let error = Error::CommandBufferFull;
        let contextual = error.context(format!("Failed to recall preset {}", preset_id));
        assert_eq!(
            contextual.to_string(),
            "Failed to recall preset 5: Command buffer is full"
        );
    }

    #[test]
    fn test_with_context_preserves_error_kind() {
        let error = Error::TransportBusy;
        let contextual = error.with_context("Operation failed");
        assert_eq!(contextual.kind(), ErrorKind::Busy);

        let error = Error::Timeout;
        let contextual = error.with_context("Operation timed out");
        assert_eq!(contextual.kind(), ErrorKind::Timeout);

        let error = Error::CommandBufferFull;
        let contextual = error.with_context("Buffer full");
        assert_eq!(contextual.kind(), ErrorKind::BufferFull);
    }

    #[test]
    fn test_nested_context() {
        // Test that context can be added to already-contextualized errors
        let error = Error::CommandBufferFull;
        let contextual1 = error.with_context("Inner context");
        let contextual2 = contextual1.with_context("Outer context");

        // Should preserve retry intelligence through multiple layers
        assert!(contextual2.is_retryable());
        assert_eq!(
            contextual2.suggested_retry_delay(),
            Some(Duration::from_millis(200))
        );

        // Message format
        assert_eq!(
            contextual2.to_string(),
            "Outer context: Inner context: Command buffer is full"
        );
    }

    #[test]
    fn test_with_context_all_retryable_types() {
        // Test all retryable error types preserve their retry metadata
        let retryable_errors = vec![
            (Error::CommandBufferFull, Duration::from_millis(200)),
            (Error::NoSocket, Duration::from_millis(200)),
            (
                Error::RuntimeQueueFull { capacity: 8 },
                Duration::from_millis(200),
            ),
            (Error::TransportBusy, Duration::from_millis(50)),
            (Error::CommandPending, Duration::from_millis(50)),
            (Error::Timeout, Duration::from_secs(2)),
        ];

        for (error, expected_delay) in retryable_errors {
            let contextual = error.with_context("Test operation");
            assert!(
                contextual.is_retryable(),
                "Context should preserve retryable: {}",
                contextual
            );
            assert_eq!(
                contextual.suggested_retry_delay(),
                Some(expected_delay),
                "Context should preserve retry delay: {}",
                contextual
            );
        }
    }

    #[test]
    fn test_runtime_queue_full_error() {
        let error = Error::RuntimeQueueFull { capacity: 64 };

        // Should have BufferFull kind
        assert_eq!(error.kind(), ErrorKind::BufferFull);

        // Should be retryable
        assert!(error.is_retryable());

        // Should have suggested retry delay (same as CommandBufferFull)
        assert_eq!(
            error.suggested_retry_delay(),
            Some(Duration::from_millis(200))
        );

        // Error message should include capacity
        let message = error.to_string();
        assert!(
            message.contains("64"),
            "Error message should include capacity"
        );
        assert!(
            message.contains("Runtime queue full"),
            "Error message should indicate queue full"
        );

        // Context should preserve retry intelligence
        let contextual = error.with_context("Failed to submit command");
        assert!(contextual.is_retryable());
        assert_eq!(contextual.kind(), ErrorKind::BufferFull);
    }
}
