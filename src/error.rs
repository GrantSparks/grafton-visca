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
//! | A caller's wait on an admitted operation, cancellation, command, or inquiry expires while the owner still holds the request | Any | Any | [`Error::ObservationTimeout`] (stage `Observation`, certainty `StillLive`) | `false` | Never resubmit: wait again on the handle, or reconcile. `is_retryable()` is `false`. |
//! | The engine's protocol lifecycle reaches a terminal deadline without correlation ambiguity (ACK, completion, inquiry reply, or retry budget) | Sony, or a raw inquiry | Any | [`Error::Timeout`] (stage `Terminal`; certainty `Unconfirmed` for a sent command, `FailedConclusively` for an inquiry, `NotAccepted` if never sent) | `false` | Follow [`Error::failure_context`]: reconcile an unconfirmed command, retry a conclusively failed inquiry. A timeout alone is not liveness proof. |
//! | An admission deadline expires before the owner accepts the request | Any | Any | [`Error::Timeout`] (stage `PreAdmission`, certainty `NotAccepted`) | `false` | The request never existed; submitting it again is safe. |
//! | A sent command's ACK/completion becomes unconfirmable under the default policy, including a STOP written inside a `NoReply` command's hold whose only answer was a socketless error that command could also have sent (it binds to neither, #795) | Raw | Datagram or stream | [`Error::UnsequencedCommandUnconfirmed`] ([`ErrorKind::Unconfirmed`]) | `false` | Never replay blindly; reconcile that command's camera effect. Whole and fragmented late bytes have the same verdict: a retained stream prefix gets a bounded grace, then is discarded as malformed rather than poisoning by segmentation. |
//! | A recorded cancellation cannot be resolved before its correlation deadline | Sony, or raw with strict policy off | Datagram or stream | [`Error::CancellationUnconfirmed`] ([`ErrorKind::Unconfirmed`]) | `false` | Reconcile the original command; cancellation was requested, not proven. |
//! | Raw command/cancellation uncertainty under strict policy | Raw | Datagram or stream | [`Error::StreamPoisoned`] | `true` | Replace the session, re-query state, and restore deliberately. |
//! | An inquiry (or user `CompletionOnly` raw command) to a camera whose earlier inquiry ended unanswered after its bytes entered the stream (that one is written once and fails at its reply deadline with the `Terminal`/`FailedConclusively` timeout above), once its owed reply has still not arrived when the profile's ambiguity window ends — a stalled connection, a camera that never answers that inquiry, or an absent address | Raw | Stream | [`Error::InquiryCorrelationLost`] (stage `Terminal`, certainty `NotAccepted`; not written, not retryable) | `false` | Inquiries to that camera cannot be correlated until the owed reply arrives or a later answer from the camera proves it never will; close and reopen the session to recover them sooner. A raw `CompletionOnly` command left unanswered holds inquiries back the same way, until its completion arrives. Other cameras and the session keep working, and commands and stops to that camera are still sent; settlement polling, `is_moving`, and `wait_until_idle` on it fail. Frames are resolved in write order (#795): the owed reply (data or a socketless rejection) settles the lane, and so does the ACK or rejection of any command written after the inquiry, since the camera answers in order — a camera that never answers that inquiry reopens its inquiries with its next command answer. |
//! | An ordinary ACK-bearing, `CompletionOnly`, or `NoReply` command to a camera whose earlier command ended [`Error::UnsequencedCommandUnconfirmed`] before its ACK (or completion) after its bytes entered the stream, once that owed answer has still not arrived when the owing command's ambiguity window ends; or a `CompletionOnly` command to a camera that has not answered anything since a `NoReply` command whose ambiguity window has ended | Raw | Stream | [`Error::CommandCorrelationLost`] (stage `Terminal`, certainty `NotAccepted`; not written, not retryable) | `false` | Commands to that camera cannot be correlated until the owed answer arrives or a later answer from the camera proves it never will (the reply to a later inquiry, or the answer to a later STOP); close and reopen the session to recover them sooner. Within the window such commands wait; a late answer is discarded and never acknowledges or completes a later command. A `CompletionOnly` command's owed completion is settled only by itself: if it never comes — a camera that drops it, or a socketless error that it or a STOP behind it could have sent that was in fact its rejection but that the ledger could not prove so within three hand-offs — ordinary commands to that camera stay latched until the session is reopened. STOPs (including the owner halt), inquiries, other cameras, and the session keep working (#795). |
//! | Request or cancellation send fails | Any | Datagram | The transport error (a terminal-looking custom error is normalized to [`Error::TransportError`]) | `false` | Treat it as this transmission's failure; the receive side remains the authority on session death. |
//! | Request or cancellation send fails after unknown stream progress | Any | Stream | [`Error::StreamPoisoned`] | `true` | Replace the session; stream position may be unknowable. |
//! | Fatal receive closure, including EOF/reset/broken pipe | Any | Datagram or stream | [`Error::ConnectionClosed`] | `true` | Replace the session and re-query state. |
//! | Transient receive fault or an idle/no-data read | Any | Datagram or stream | No immediate public failure; request policy/deadlines continue | n/a | Keep driving the session; use a bounded application heartbeat for silent peers. |
//! | Framer overflow or unrecoverable discard/resynchronization failure | Any | Stream | [`Error::StreamPoisoned`] | `true` | Replace the session. |
//! | Local request admission is full | Any | Any | [`Error::RuntimeQueueFull`] | `false` | Back off until admission capacity is available. An urgent typed STOP can still use its target's control reserve. |
//! | An urgent typed STOP finds its target's control reserve and ordinary admission both full | Any | Any | [`Error::ControlReserveExhausted`] | `false` | Back off briefly: earlier stops for that camera are still pending. |
//! | Camera returns a conclusive protocol rejection before ACK | Any | Any | The exact VISCA error variant (a typed STOP's `0x41` is reported at once, never retried) | `false` | Apply the variant's retry policy; camera state and socket routing remain authoritative. |
//! | Camera reports an error for a command it already acknowledged | Any | Any | [`Error::CommandFailedAfterAck`] wrapping the exact VISCA error (stage `Terminal`, certainty `Unconfirmed`; not retryable); the engine never writes the command again | `false` | The command may have partly executed: reconcile before resubmitting a relative move or preset (#795). |
//! | An owner halt supersedes declared motion | Any | Any | [`Error::MotionSuperseded`] (stage `PreAdmission` or `Terminal`; certainty `NotAccepted` if it never reached the camera, `Unconfirmed` if an earlier attempt may have) | `false` | Resubmit only on `NotAccepted`; otherwise reconcile. Not proof of physical rest (#795). |
//! | Application closes the owner | Any | Any | [`Error::RuntimeShutdown`] | `false` | Reconnect only if the application intends to start another session. |

use thiserror::Error as ThisError;

use std::{borrow::Cow, convert::Infallible, fmt, io, sync::Arc, time::Duration};

use crate::OperationId;

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
/// ## Retryable and Non-Retryable Errors
/// Every variant has one [`ErrorKind`] ([`Error::kind()`]). Retryability is a
/// function of that kind: [`ErrorKind::Timeout`], [`ErrorKind::BufferFull`],
/// [`ErrorKind::Busy`] and [`ErrorKind::Transport`] describe temporary
/// conditions, apart from the few variants whose new attempt would loop or
/// duplicate a live request (see [`Error::suggested_retry_delay()`]). Use
/// [`Error::is_retryable()`] to check whether an error can be retried, and
/// [`Error::suggested_retry_delay()`] for the recommended delay. Whether a
/// resubmission could duplicate a physical effect is a separate question that
/// [`Error::failure_context`] answers.
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
    #[non_exhaustive]
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
    #[non_exhaustive]
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
    #[non_exhaustive]
    InvalidResponse {
        /// Description of expected response.
        expected: Cow<'static, str>,
        /// Actual bytes received.
        actual: Vec<u8>,
    },

    /// Camera model doesn't support the requested feature.
    #[error("Feature '{feature}' not supported by this camera model")]
    #[non_exhaustive]
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
    ///
    /// Before the camera acknowledged the command, this is a conclusive
    /// rejection: the command did not start. The owner retries it for
    /// ordinary movement, but never for a typed STOP (for example a focus
    /// STOP while auto-focus owns the lens). After an acknowledgement it ends
    /// the command without any resend, and the command may have partly
    /// executed; reconcile before resubmitting a relative move or preset.
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
    #[non_exhaustive]
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
    #[error("Unknown error code: {0:#04X}")]
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
    #[non_exhaustive]
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
    #[non_exhaustive]
    BufferTooSmall {
        /// Required buffer size.
        required: usize,
        /// Actual buffer size provided.
        actual: usize,
    },

    /// Parameter value is out of the acceptable range.
    #[error("Parameter out of range: {parameter} = {value} (valid range: {min}..={max})")]
    #[non_exhaustive]
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

    /// A deadline expired.
    ///
    /// `context` says which deadline (the [`FailureStage`]) and what is known
    /// about the request's effect (the [`Certainty`]): an admission deadline
    /// (`PreAdmission`, `NotAccepted`), the engine's protocol lifecycle
    /// (`Terminal`), a cancellation's resolution (`CancellationAttempt`), or
    /// the session's connection (`Session`). A caller's own wait on a request
    /// the owner still holds is [`Self::ObservationTimeout`] instead.
    ///
    /// A transport reports an idle read with this variant; owners treat that
    /// as "no data" rather than as a failure.
    #[error("Operation timed out ({})", context.stage)]
    #[non_exhaustive]
    Timeout {
        /// Which deadline expired, and what is known about the request.
        context: FailureContext,
    },

    /// A caller's wait expired while the owner still holds the request.
    ///
    /// The request — an operation, a cancellation, a command, or an
    /// inquiry — keeps running under its own deadlines and may still take
    /// effect. Never resubmit it: wait again on the operation handle, or
    /// reconcile the camera's state. [`Self::is_retryable`] is `false`.
    #[error("Wait for operation {operation} timed out; it is still running")]
    #[non_exhaustive]
    ObservationTimeout {
        /// The admitted request the wait observed.
        operation: OperationId,
    },

    /// An owner halt superseded older declared motion before it finished.
    ///
    /// `context` reports what is known about the superseded request. Its
    /// certainty is [`Certainty::NotAccepted`] when no attempt was ever
    /// written, or when every written attempt was conclusively rejected by
    /// the camera; resubmitting it is then safe. It is
    /// [`Certainty::Unconfirmed`] when an earlier attempt may have reached
    /// the camera; reconcile before resubmitting. The stage is
    /// [`FailureStage::PreAdmission`] when the fence rejected the submission
    /// itself and [`FailureStage::Terminal`] when it ended an admitted
    /// request. Supersession is never proof of physical rest.
    #[error("Motion on {axes:?} was superseded by an owner halt ({})", context.stage)]
    #[non_exhaustive]
    MotionSuperseded {
        /// Axes selected by the fence.
        axes: crate::AffectedAxes,
        /// Where the request was superseded, and whether it may have taken
        /// effect.
        context: FailureContext,
    },

    /// A later admitted operation prevents attribution of polled settlement.
    #[error("Settlement of operation {operation} was superseded on {axes:?}")]
    #[non_exhaustive]
    SettlementSuperseded {
        /// The original operation.
        operation: OperationId,
        /// Its affected axes.
        axes: crate::AffectedAxes,
    },

    /// An applied operation's settlement could not be observed. The underlying
    /// inquiry failure never authorizes replay of the original movement.
    #[error("Could not observe settlement of operation {operation}: {source}")]
    #[non_exhaustive]
    SettlementObservationFailed {
        /// The already-applied operation being observed.
        operation: OperationId,
        /// The exact polling failure, including its inquiry/transport context.
        #[source]
        source: Box<Error>,
    },

    /// Maximum retry attempts exceeded.
    #[error("Maximum retries exceeded")]
    MaxRetriesExceeded,

    /// Operation is not supported by this implementation.
    #[error("Operation not supported")]
    NotSupported,

    /// Operation cannot be performed in current state.
    #[error("Invalid state: {0}")]
    InvalidState(Cow<'static, str>),

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

    /// Raw-VISCA inquiries to `camera` cannot be correlated on this session,
    /// so this request was not sent.
    ///
    /// On a stream transport (TCP, serial) an inquiry that timed out after it
    /// was written is still owed its reply: the stream loses nothing, and raw
    /// inquiry replies carry no identity, so that late reply would otherwise be
    /// returned as the next inquiry's data. The session waits the profile's
    /// [`ambiguity_timeout`](crate::profile::ProfileTiming::ambiguity_timeout)
    /// for it. If it has not arrived by then — a long stall, a camera that
    /// never answers that inquiry, or an absent address — every inquiry to
    /// that camera (and every user-declared
    /// [`CompletionOnly`](crate::raw::RawReplyShape::CompletionOnly) raw
    /// command, whose completion is equally unkeyed) fails with this error
    /// without being written, until the owed reply arrives or a later answer
    /// proves it never will.
    ///
    /// Nothing was sent, so the failure context is `Terminal` /
    /// [`Certainty::NotAccepted`]. It is not retryable on this session: retrying
    /// fails the same way until the lane reopens. The session itself is live —
    /// other cameras are unaffected, and STOPs and other ACK-bearing commands
    /// to this camera are still sent; settlement polling and motion
    /// observation on this camera fail because they need inquiries — so
    /// [`Self::requires_new_session`] is `false`. Frames on a raw stream are
    /// resolved in write order, and a camera answers in the order it reads:
    /// the owed reply (data or a socketless rejection) reopens the lane, and
    /// so does the first answer to any command written after the inquiry,
    /// which proves the owed reply will never come. A camera that never
    /// answers that inquiry therefore keeps accepting commands, and its next
    /// command answer reopens its inquiries. To recover this camera's
    /// inquiries sooner, close and reopen the session. A raw
    /// [`CompletionOnly`](crate::raw::RawReplyShape::CompletionOnly) command
    /// whose answer is still owed holds the camera's inquiries back the same
    /// way: it is exclusive on its camera until its completion arrives.
    #[error(
        "Raw inquiry correlation for {camera} is lost: an earlier inquiry's reply is still \
         owed by the stream, so this request was not sent; reopen the session to recover"
    )]
    #[non_exhaustive]
    InquiryCorrelationLost {
        /// The camera whose raw inquiry lane is latched.
        camera: crate::CameraId,
    },

    /// A raw-TCP (byte-stream) camera still owes the answer to an earlier
    /// command, so this command was not sent (#795).
    ///
    /// The command half of [`Self::InquiryCorrelationLost`]. A stream loses
    /// nothing it accepted, so a command that ended
    /// [`Self::UnsequencedCommandUnconfirmed`] before the camera's ACK (or,
    /// for a [`CompletionOnly`](crate::raw::RawReplyShape::CompletionOnly)
    /// command, its completion) will still be answered, ahead of any later
    /// command, however long a stall delays it. Raw answers carry no identity,
    /// so that late answer would otherwise acknowledge and complete the next
    /// command. The session discards it on arrival and holds later ordinary
    /// commands to the camera for the profile's
    /// [`ambiguity_timeout`](crate::profile::ProfileTiming::ambiguity_timeout).
    /// If it has not arrived by then, every ordinary ACK-bearing and
    /// `CompletionOnly` command to that camera fails with this error without
    /// being written, until it arrives or a later answer from the camera (the
    /// reply to a later inquiry, or the answer to a later STOP) proves it
    /// never will; a `CompletionOnly` command's completion, which comes only
    /// once it has run, is settled only by itself.
    ///
    /// Nothing was sent, so the failure context is `Terminal` /
    /// [`Certainty::NotAccepted`]. It is not retryable on this session until
    /// the lane reopens. A `NoReply` command to that camera fails with it
    /// too: its possible rejection could not be told from the owed answer.
    /// Conversely a `NoReply` command's own possible rejection, a socketless
    /// `z0 60 41 FF` that a stall can delay past any window, stays owed until
    /// a later answer from the camera settles it: past that command's
    /// ambiguity window a `CompletionOnly` command to the camera, whose own
    /// rejection could not be told from it, fails with this error too.
    /// STOPs (including an owner halt), inquiries, and other cameras are never
    /// blocked, and the session stays
    /// live, so [`Self::requires_new_session`] is `false`. To recover this
    /// camera's commands without waiting, close and reopen the session.
    #[error(
        "Raw command correlation for {camera} is lost: an earlier command's answer is still \
         owed by the stream, so this request was not sent; reopen the session to recover"
    )]
    #[non_exhaustive]
    CommandCorrelationLost {
        /// The camera whose raw command lane is latched.
        camera: crate::CameraId,
    },

    /// The camera reported an error for a command it had already
    /// acknowledged (#795).
    ///
    /// The ACK proved the camera accepted the command, so it may have started
    /// or partly executed before failing: a relative move or preset recall may
    /// already have moved. `source` is the camera's exact error. The engine
    /// never writes the command again, and this error is not retryable and
    /// reports `Terminal` / [`Certainty::Unconfirmed`] whatever the camera's
    /// code would mean before an ACK. Reconcile the camera state before
    /// resubmitting.
    #[error("Camera reported an error after acknowledging the command: {source}")]
    #[non_exhaustive]
    CommandFailedAfterAck {
        /// The camera's error, exactly as it would be reported before an ACK.
        #[source]
        source: Box<Error>,
    },

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
    #[non_exhaustive]
    RuntimeQueueFull {
        /// The maximum queue capacity that was reached.
        capacity: usize,
    },

    /// An urgent typed STOP found its target's control reserve full while
    /// ordinary admission was full too (D26, #778).
    ///
    /// Each registered camera has a small admission reserve, one slot per
    /// typed STOP its profile supports, that ordinary work cannot use. This is
    /// returned only when that reserve is already held by earlier stops and
    /// no ordinary slot is free either. Back off briefly; an earlier stop for
    /// the same camera is still pending.
    #[error("Control reserve for camera {target} is full ({reserve} slots)")]
    #[non_exhaustive]
    ControlReserveExhausted {
        /// The camera whose reserve is full.
        target: crate::CameraId,
        /// The size of that camera's reserve.
        reserve: usize,
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
    #[non_exhaustive]
    StreamPoisoned {
        /// Description of why the transport was poisoned.
        reason: Cow<'static, str>,
    },

    /// Invalid camera ID provided.
    #[error("Invalid camera ID {id}: must be 1-7 for individual cameras or 8 for broadcast")]
    #[non_exhaustive]
    InvalidCameraId {
        /// The invalid camera ID that was provided.
        id: u8,
    },

    /// Response exceeds maximum allowed size.
    #[error("Response too large: exceeds maximum of {max_size} bytes")]
    #[non_exhaustive]
    ResponseTooLarge {
        /// Maximum allowed size.
        max_size: usize,
    },

    /// Cancellation was requested for an inquiry.
    ///
    /// Only operations that occupy a VISCA socket can be canceled. An inquiry
    /// holds no socket and completes when its reply arrives, so canceling its
    /// handle fails with this error; wait for the reply or drop the handle.
    #[error("Inquiries cannot be canceled: an inquiry holds no VISCA socket and completes with its reply")]
    InquiryNotCancelable,

    /// Invalid network address format.
    #[error("Invalid address: {reason}")]
    #[non_exhaustive]
    InvalidAddress {
        /// Reason why the address is invalid.
        reason: Cow<'static, str>,
    },

    /// Selected standard transport is unsupported for the selected built-in profile.
    #[error("Profile {profile} does not support {transport} transport")]
    #[non_exhaustive]
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
    #[non_exhaustive]
    WithContext {
        /// Human-readable context describing what operation failed.
        context: Cow<'static, str>,
        /// The underlying error that occurred.
        source: Box<Error>,
    },
}

/// Where a deadline or certainty failure happened, and what is known about
/// the affected request's effect. Read it with [`Error::failure_context`].
///
/// # Example
///
/// ```rust
/// use grafton_visca::{Certainty, Error, FailureStage};
///
/// /// What to do after a failed submission or wait.
/// enum Next {
///     Resubmit,
///     WaitAgain,
///     Reconcile,
///     GiveUp(Error),
/// }
///
/// fn next_step(error: Error) -> Next {
///     match error.failure_context() {
///         Some(context) => match context.certainty {
///             Certainty::NotAccepted | Certainty::FailedConclusively => Next::Resubmit,
///             Certainty::StillLive if context.stage == FailureStage::Observation => {
///                 Next::WaitAgain
///             }
///             Certainty::StillLive | Certainty::Unconfirmed => Next::Reconcile,
///             _ => Next::GiveUp(error),
///         },
///         None => Next::GiveUp(error),
///     }
/// }
///
/// assert!(matches!(next_step(Error::io_timeout()), Next::Reconcile));
/// assert!(matches!(
///     next_step(Error::UnsequencedCommandUnconfirmed),
///     Next::Reconcile
/// ));
/// assert!(matches!(next_step(Error::SyntaxError), Next::GiveUp(_)));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct FailureContext {
    /// Which lifecycle stage the failure belongs to.
    pub stage: FailureStage,
    /// What is known about whether the request took effect.
    pub certainty: Certainty,
}

impl FailureContext {
    /// A context with the given stage and certainty.
    #[must_use]
    pub const fn new(stage: FailureStage, certainty: Certainty) -> Self {
        Self { stage, certainty }
    }
}

/// The lifecycle stage a failure belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FailureStage {
    /// Before the owner admitted the request.
    PreAdmission,
    /// A caller's wait: on a request the owner still holds, or on the
    /// camera's state through read-only inquiries (settlement, idle and
    /// motion queries).
    Observation,
    /// The engine's protocol lifecycle for the request: ACK, completion,
    /// inquiry reply, retry budget, or correlation ambiguity.
    Terminal,
    /// The resolution of a cancellation the owner accepted.
    CancellationAttempt,
    /// The session's connection: connecting, handshaking, or one transport
    /// read or write.
    Session,
}

impl fmt::Display for FailureStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::PreAdmission => "before admission",
            Self::Observation => "while observing",
            Self::Terminal => "in the protocol lifecycle",
            Self::CancellationAttempt => "while resolving a cancellation",
            Self::Session => "on the connection",
        })
    }
}

/// What is known about whether a request took effect.
///
/// Submitting the same request again is safe only after [`Self::NotAccepted`]
/// or [`Self::FailedConclusively`]: in both, the request had no effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Certainty {
    /// The request was never accepted and has no effect. Submitting it again
    /// is safe.
    NotAccepted,
    /// The owner still holds the request, which may yet take effect. Wait
    /// again or reconcile; never resubmit.
    StillLive,
    /// The request ended without effect.
    FailedConclusively,
    /// The requested physical outcome is unconfirmed. At the observation stage,
    /// application may already be known while settlement is not. Reconcile
    /// the camera's state before deciding to submit it again.
    Unconfirmed,
}

impl Error {
    /// Reports a failed connection attempt.
    #[must_use]
    pub fn connection_failed(addr: impl Into<Cow<'static, str>>, source: Arc<io::Error>) -> Self {
        Self::ConnectionFailed {
            addr: addr.into(),
            source,
        }
    }

    /// Reports a closed connection with an optional cause.
    #[must_use]
    pub const fn connection_closed(reason: Option<Cow<'static, str>>) -> Self {
        Self::ConnectionClosed { reason }
    }

    /// Reports bytes that do not match the expected response.
    #[must_use]
    pub fn invalid_response(expected: impl Into<Cow<'static, str>>, actual: Vec<u8>) -> Self {
        Self::InvalidResponse {
            expected: expected.into(),
            actual,
        }
    }

    /// Reports a feature unsupported by the selected camera.
    #[must_use]
    pub const fn feature_not_supported(feature: &'static str) -> Self {
        Self::FeatureNotSupported { feature }
    }

    /// Reports an invalid parameter and its diagnostic value.
    #[must_use]
    pub fn invalid_parameter(
        parameter: &'static str,
        value: impl Into<Cow<'static, str>>,
        reason: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::InvalidParameter {
            parameter,
            value: value.into(),
            reason: reason.into(),
        }
    }

    /// Reports a numeric parameter outside its inclusive bounds.
    #[must_use]
    pub const fn parameter_out_of_range(
        parameter: &'static str,
        value: i32,
        min: i32,
        max: i32,
    ) -> Self {
        Self::ParameterOutOfRange {
            parameter,
            value,
            min,
            max,
        }
    }

    /// Reports an encoding buffer smaller than required.
    #[must_use]
    pub const fn buffer_too_small(required: usize, actual: usize) -> Self {
        Self::BufferTooSmall { required, actual }
    }

    /// Reports a caller wait that expired while the operation remains admitted.
    #[must_use]
    pub const fn observation_timeout(operation: OperationId) -> Self {
        Self::ObservationTimeout { operation }
    }

    /// Reports declared motion superseded by an owner halt, with what is
    /// known about whether it took effect.
    #[must_use]
    pub const fn motion_superseded(axes: crate::AffectedAxes, context: FailureContext) -> Self {
        Self::MotionSuperseded { axes, context }
    }

    /// Reports a polling failure after the movement was applied.
    #[must_use]
    pub fn settlement_observation_failed(
        operation: OperationId,
        source: impl Into<Box<Error>>,
    ) -> Self {
        Self::SettlementObservationFailed {
            operation,
            source: source.into(),
        }
    }

    /// Reports a camera whose raw inquiries cannot be correlated on this
    /// session.
    #[must_use]
    pub const fn inquiry_correlation_lost(camera: crate::CameraId) -> Self {
        Self::InquiryCorrelationLost { camera }
    }

    /// Reports a camera whose raw commands cannot be correlated on this
    /// session until an owed answer arrives.
    #[must_use]
    pub const fn command_correlation_lost(camera: crate::CameraId) -> Self {
        Self::CommandCorrelationLost { camera }
    }

    /// Reports a camera error for a command the camera had acknowledged.
    #[must_use]
    pub fn command_failed_after_ack(source: impl Into<Box<Error>>) -> Self {
        Self::CommandFailedAfterAck {
            source: source.into(),
        }
    }

    /// Reports exhausted ordinary admission capacity.
    #[must_use]
    pub const fn runtime_queue_full(capacity: usize) -> Self {
        Self::RuntimeQueueFull { capacity }
    }

    /// Reports an exhausted urgent control reserve for one camera.
    #[must_use]
    pub const fn control_reserve_exhausted(target: crate::CameraId, reserve: usize) -> Self {
        Self::ControlReserveExhausted { target, reserve }
    }

    /// Reports a stream whose framing or write position became unknowable.
    #[must_use]
    pub fn stream_poisoned(reason: impl Into<Cow<'static, str>>) -> Self {
        Self::StreamPoisoned {
            reason: reason.into(),
        }
    }

    /// Reports an invalid VISCA camera address.
    #[must_use]
    pub const fn invalid_camera_id(id: u8) -> Self {
        Self::InvalidCameraId { id }
    }

    /// Reports a response exceeding the configured size limit.
    #[must_use]
    pub const fn response_too_large(max_size: usize) -> Self {
        Self::ResponseTooLarge { max_size }
    }

    /// Reports an invalid transport address.
    #[must_use]
    pub fn invalid_address(reason: impl Into<Cow<'static, str>>) -> Self {
        Self::InvalidAddress {
            reason: reason.into(),
        }
    }

    /// Reports a transport unsupported by a built-in profile.
    #[must_use]
    pub const fn unsupported_transport(
        profile: crate::camera::profiles::ProfileId,
        transport: crate::camera::TransportKind,
    ) -> Self {
        Self::UnsupportedTransport { profile, transport }
    }

    /// Reports that later admitted motion superseded this operation's settlement.
    #[must_use]
    pub const fn settlement_superseded(operation: OperationId, axes: crate::AffectedAxes) -> Self {
        Self::SettlementSuperseded { operation, axes }
    }

    /// Get the kind of this error for categorized handling.
    ///
    /// # Examples
    /// ```rust
    /// use grafton_visca::{Error, ErrorKind};
    ///
    /// let error = Error::CommandBufferFull;
    /// assert_eq!(error.kind(), ErrorKind::BufferFull);
    /// ```
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        match self {
            // Timeout: transient timing failures
            Self::Timeout { .. } | Self::ObservationTimeout { .. } | Self::MaxRetriesExceeded => {
                ErrorKind::Timeout
            }

            // Cancelled: explicit cancellation
            Self::CommandCanceled => ErrorKind::Cancelled,

            // BufferFull: transient capacity exhaustion
            Self::CommandBufferFull
            | Self::RuntimeQueueFull { .. }
            | Self::ControlReserveExhausted { .. }
            | Self::NoSocket => ErrorKind::BufferFull,

            // NotExecutable: command invalid in current state
            Self::CommandNotExecutable
            | Self::InvalidState(..)
            | Self::InquiryCorrelationLost { .. }
            | Self::CommandCorrelationLost { .. } => ErrorKind::NotExecutable,

            // IoClosed: connection/transport no longer usable
            Self::ConnectionClosed { .. } | Self::RuntimeShutdown | Self::StreamPoisoned { .. } => {
                ErrorKind::IoClosed
            }

            // Transport: one failed operation on a live session.
            Self::TransportError(..) => ErrorKind::Transport,

            // Unconfirmed: one transmitted operation has an unknowable result
            Self::CancellationUnconfirmed
            | Self::UnsequencedCommandUnconfirmed
            | Self::CommandFailedAfterAck { .. } => ErrorKind::Unconfirmed,

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
            | Self::ResponseTooLarge { .. } => ErrorKind::Protocol,

            // Unsupported: feature/capability not available
            Self::FeatureNotSupported { .. } | Self::NotSupported | Self::MissingRuntime => {
                ErrorKind::Unsupported
            }

            // InvalidParameter: caller-supplied value is invalid
            Self::InvalidParameter { .. }
            | Self::ParameterOutOfRange { .. }
            | Self::SyntaxError
            | Self::InvalidRequest(..)
            | Self::BufferTooSmall { .. }
            | Self::InvalidCameraId { .. }
            | Self::InquiryNotCancelable { .. }
            | Self::InvalidAddress { .. } => ErrorKind::InvalidParameter,
            Self::UnsupportedTransport { .. } => ErrorKind::Unsupported,

            // Busy: transient contention
            Self::CommandPending => ErrorKind::Busy,

            // Other: truly uncategorizable
            Self::RuntimeIdentityExhausted => ErrorKind::Other,

            // Delegated: unwrap context wrapper
            Self::WithContext { source, .. } | Self::SettlementObservationFailed { source, .. } => {
                source.kind()
            }
            Self::MotionSuperseded { .. } | Self::SettlementSuperseded { .. } => {
                ErrorKind::Cancelled
            }

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
    /// let dropped = Error::connection_closed(None);
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
            Self::WithContext { source, .. } | Self::SettlementObservationFailed { source, .. } => {
                source.requires_new_session()
            }

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
            | Self::ParameterOutOfRange { .. }
            | Self::Timeout { .. }
            | Self::ObservationTimeout { .. }
            | Self::MotionSuperseded { .. }
            | Self::SettlementSuperseded { .. }
            | Self::MaxRetriesExceeded
            | Self::NotSupported
            | Self::InvalidState(..)
            | Self::CancellationUnconfirmed
            | Self::UnsequencedCommandUnconfirmed
            | Self::InquiryCorrelationLost { .. }
            | Self::CommandCorrelationLost { .. }
            | Self::CommandFailedAfterAck { .. }
            | Self::RuntimeIdentityExhausted
            | Self::RuntimeQueueFull { .. }
            | Self::ControlReserveExhausted { .. }
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
    /// Exactly the errors with a [`Self::suggested_retry_delay()`] are
    /// retryable: the temporary [`ErrorKind`]s (`Timeout`, `BufferFull`,
    /// `Busy`, `Transport`), except an error whose new attempt would loop or
    /// duplicate a live request.
    ///
    /// This classifies the *condition*, not replay safety. Whether submitting
    /// the same request again could duplicate a physical effect is a separate
    /// question that [`Self::failure_context`] answers: only
    /// [`Certainty::NotAccepted`] and [`Certainty::FailedConclusively`] are
    /// replay-safe.
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
        self.suggested_retry_delay().is_some()
    }

    /// Get a suggested retry delay for retryable errors.
    ///
    /// Returns `Some(Duration)` with a recommended delay before retrying the
    /// operation, or `None` if the error is not retryable. The delay depends
    /// only on [`Self::kind()`]:
    ///
    /// - [`ErrorKind::Busy`] and [`ErrorKind::Transport`]: 50 ms, for camera
    ///   contention or one failed transport operation on a live session.
    /// - [`ErrorKind::BufferFull`]: 200 ms, for camera or local capacity to
    ///   free up.
    /// - [`ErrorKind::Timeout`]: 2 s, to allow more time.
    ///
    /// Three errors of a retryable kind have no delay, because a new attempt
    /// would loop or duplicate a live request: [`Self::MaxRetriesExceeded`]
    /// (the retry budget is already spent), [`Self::ObservationTimeout`] and
    /// [`Self::SettlementObservationFailed`] (the request is still running or
    /// already applied). [`Self::WithContext`] reports its source's delay.
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
        /// Delay for camera contention or one failed live-session transport operation.
        const CONTENTION_RETRY_DELAY: Duration = Duration::from_millis(50);
        /// Delay for camera or local capacity to free up.
        const CAPACITY_RETRY_DELAY: Duration = Duration::from_millis(200);
        /// Delay after an expired deadline.
        const TIMEOUT_RETRY_DELAY: Duration = Duration::from_secs(2);

        match self {
            Self::WithContext { source, .. } => source.suggested_retry_delay(),
            // A retryable kind, but a new attempt would loop or duplicate a
            // live request.
            Self::MaxRetriesExceeded
            | Self::ObservationTimeout { .. }
            | Self::SettlementObservationFailed { .. } => None,
            _ => match self.kind() {
                ErrorKind::Busy | ErrorKind::Transport => Some(CONTENTION_RETRY_DELAY),
                ErrorKind::BufferFull => Some(CAPACITY_RETRY_DELAY),
                ErrorKind::Timeout => Some(TIMEOUT_RETRY_DELAY),
                ErrorKind::Cancelled
                | ErrorKind::NotExecutable
                | ErrorKind::IoClosed
                | ErrorKind::IoRefused
                | ErrorKind::Unconfirmed
                | ErrorKind::Protocol
                | ErrorKind::Unsupported
                | ErrorKind::InvalidParameter
                | ErrorKind::Other => None,
            },
        }
    }

    /// Where a deadline or certainty failure happened, and what is known
    /// about the affected request's effect.
    ///
    /// Returns `Some` for every timeout ([`Self::Timeout`],
    /// [`Self::ObservationTimeout`]), for the unconfirmed outcomes
    /// ([`Self::UnsequencedCommandUnconfirmed`],
    /// [`Self::CancellationUnconfirmed`], and settlement observation
    /// failures), and for [`Self::MotionSuperseded`], which carries the
    /// context recorded when the halt superseded it (`NotAccepted` when it
    /// never reached the camera); `None` otherwise. [`Self::WithContext`] is
    /// looked through. [`Self::SettlementObservationFailed`] reports
    /// uncertainty for the original movement while retaining the polling
    /// failure as its source.
    ///
    /// # Example
    ///
    /// ```rust
    /// use grafton_visca::{Certainty, Error, FailureStage};
    ///
    /// fn may_resubmit(error: &Error) -> bool {
    ///     error
    ///         .failure_context()
    ///         .is_some_and(|context| context.certainty == Certainty::NotAccepted)
    /// }
    ///
    /// let unconfirmed = Error::UnsequencedCommandUnconfirmed;
    /// assert!(!may_resubmit(&unconfirmed));
    /// assert_eq!(
    ///     unconfirmed.failure_context().map(|context| context.stage),
    ///     Some(FailureStage::Terminal)
    /// );
    /// ```
    #[must_use]
    pub fn failure_context(&self) -> Option<FailureContext> {
        match self {
            Self::Timeout { context } => Some(*context),
            Self::SettlementObservationFailed { .. } | Self::SettlementSuperseded { .. } => Some(
                FailureContext::new(FailureStage::Observation, Certainty::Unconfirmed),
            ),
            Self::MotionSuperseded { context, .. } => Some(*context),
            Self::ObservationTimeout { .. } => Some(FailureContext::new(
                FailureStage::Observation,
                Certainty::StillLive,
            )),
            Self::UnsequencedCommandUnconfirmed => Some(FailureContext::new(
                FailureStage::Terminal,
                Certainty::Unconfirmed,
            )),
            Self::CancellationUnconfirmed => Some(FailureContext::new(
                FailureStage::CancellationAttempt,
                Certainty::Unconfirmed,
            )),
            Self::InquiryCorrelationLost { .. } | Self::CommandCorrelationLost { .. } => Some(
                FailureContext::new(FailureStage::Terminal, Certainty::NotAccepted),
            ),
            Self::CommandFailedAfterAck { .. } => Some(FailureContext::new(
                FailureStage::Terminal,
                Certainty::Unconfirmed,
            )),
            Self::WithContext { source, .. } => source.failure_context(),
            _ => None,
        }
    }

    /// A timeout with the given stage and certainty.
    #[must_use]
    pub const fn timeout(stage: FailureStage, certainty: Certainty) -> Self {
        Self::Timeout {
            context: FailureContext::new(stage, certainty),
        }
    }

    /// An admission deadline expired before the owner accepted the request.
    #[cfg(any(feature = "async", feature = "blocking", test))]
    pub(crate) const fn admission_timeout() -> Self {
        Self::timeout(FailureStage::PreAdmission, Certainty::NotAccepted)
    }

    /// A read-only state query (an idle wait or a motion query) ran out of
    /// time. It changed nothing, so it can be repeated.
    #[cfg(any(feature = "async", feature = "blocking", test))]
    pub(crate) const fn query_timeout() -> Self {
        Self::timeout(FailureStage::Observation, Certainty::NotAccepted)
    }

    /// Whether a caller's own deadline produced this error: its wait expired,
    /// or its admission deadline passed. The request's protocol lifecycle did
    /// not.
    #[cfg(any(feature = "blocking", feature = "async"))]
    pub(crate) fn is_caller_deadline(&self) -> bool {
        self.failure_context().is_some_and(|context| {
            matches!(
                context.stage,
                FailureStage::Observation | FailureStage::PreAdmission
            )
        })
    }

    /// Reports a caller deadline met inside a read-only state query as the
    /// query's own repeatable timeout, whichever inquiry it interrupted.
    #[cfg(any(feature = "blocking", feature = "async"))]
    pub(crate) fn into_query_timeout(self) -> Self {
        if self.is_caller_deadline() {
            Self::query_timeout()
        } else {
            self
        }
    }

    /// A connection could not be established or handshaken in time; nothing
    /// was submitted.
    #[cfg(any(
        feature = "blocking",
        feature = "runtime-tokio",
        feature = "runtime-smol",
        feature = "transport-serial",
        feature = "transport-serial-tokio"
    ))]
    pub(crate) const fn connect_timeout() -> Self {
        Self::timeout(FailureStage::Session, Certainty::NotAccepted)
    }

    /// One transport read or write did not finish in time.
    ///
    /// This is how a transport reports an expired read or write timeout,
    /// including an idle read that received nothing, which owners treat as
    /// "no data" rather than as a failure. A write that timed out may or may
    /// not have left, so its certainty is [`Certainty::Unconfirmed`].
    #[must_use]
    pub const fn io_timeout() -> Self {
        Self::timeout(FailureStage::Session, Certainty::Unconfirmed)
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
    pub fn context<D: fmt::Display>(self, context: D) -> Self {
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
        assert_eq!(
            Error::io_timeout().to_string(),
            "Operation timed out (on the connection)"
        );
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
        assert!(Error::io_timeout().is_retryable());
        assert!(Error::RuntimeQueueFull { capacity: 8 }.is_retryable());
        assert!(Error::TransportError(Cow::Borrowed("datagram send failed")).is_retryable());

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
            Error::io_timeout().suggested_retry_delay(),
            Some(Duration::from_secs(2))
        );
        assert_eq!(
            Error::TransportError(Cow::Borrowed("datagram send failed")).suggested_retry_delay(),
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
                Error::io_timeout(),
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
                "urgent control reserve exhausted",
                Error::control_reserve_exhausted(crate::CameraId::CAMERA_2, 3),
                ErrorKind::BufferFull,
                Some(Duration::from_millis(200)),
            ),
            (
                "caller wait expired on a live request",
                Error::observation_timeout(OperationId::from_raw(7)),
                ErrorKind::Timeout,
                None,
            ),
            (
                "settlement polling failed after an applied movement",
                Error::settlement_observation_failed(OperationId::from_raw(8), Error::io_timeout()),
                ErrorKind::Timeout,
                None,
            ),
            (
                "contextual settlement polling failure",
                Error::settlement_observation_failed(
                    OperationId::from_raw(9),
                    Error::CommandBufferFull,
                )
                .with_context("wait for settled"),
                ErrorKind::BufferFull,
                None,
            ),
            (
                "motion superseded by an owner halt",
                Error::motion_superseded(
                    crate::AffectedAxes::PAN_TILT,
                    FailureContext::new(FailureStage::PreAdmission, Certainty::NotAccepted),
                ),
                ErrorKind::Cancelled,
                None,
            ),
            (
                "settlement superseded by an owner halt",
                Error::settlement_superseded(OperationId::from_raw(10), crate::AffectedAxes::ZOOM),
                ErrorKind::Cancelled,
                None,
            ),
            (
                "camera failed an acknowledged command",
                Error::command_failed_after_ack(Error::CommandNotExecutable),
                ErrorKind::Unconfirmed,
                None,
            ),
            (
                "raw stream inquiries cannot be correlated",
                Error::inquiry_correlation_lost(crate::CameraId::CAMERA_2),
                ErrorKind::NotExecutable,
                None,
            ),
            (
                "raw stream commands cannot be correlated",
                Error::command_correlation_lost(crate::CameraId::CAMERA_2),
                ErrorKind::NotExecutable,
                None,
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
        assert_eq!(Error::io_timeout().kind(), ErrorKind::Timeout);
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

    /// #795: the command lane's latch error mirrors the inquiry lane's.
    #[test]
    fn command_correlation_lost_is_unsent_live_session_and_not_retryable() {
        let error = Error::command_correlation_lost(crate::CameraId::CAMERA_2);
        assert!(matches!(
            error,
            Error::CommandCorrelationLost { camera } if camera == crate::CameraId::CAMERA_2
        ));
        assert_eq!(error.kind(), ErrorKind::NotExecutable);
        assert!(!error.is_retryable());
        assert_eq!(error.suggested_retry_delay(), None);
        assert!(!error.requires_new_session());
        assert_eq!(
            error.failure_context(),
            Some(FailureContext::new(
                FailureStage::Terminal,
                Certainty::NotAccepted
            ))
        );
        assert!(error.to_string().contains("Camera 2"), "{error}");
    }

    /// #795: a camera error after an ACK never invites a resubmission, whatever
    /// its code would mean before the ACK, and keeps the exact camera error.
    #[test]
    fn a_camera_error_after_ack_is_unconfirmed_and_not_retryable() {
        for camera in [
            Error::CommandBufferFull,
            Error::NoSocket,
            Error::CommandNotExecutable,
            Error::SyntaxError,
        ] {
            let error = Error::command_failed_after_ack(camera.clone());
            assert_eq!(error.kind(), ErrorKind::Unconfirmed, "{error:?}");
            assert!(!error.is_retryable(), "{error:?}");
            assert_eq!(error.suggested_retry_delay(), None, "{error:?}");
            assert!(!error.requires_new_session(), "{error:?}");
            assert_eq!(
                error.failure_context(),
                Some(FailureContext::new(
                    FailureStage::Terminal,
                    Certainty::Unconfirmed
                ))
            );
            assert_eq!(
                std::error::Error::source(&error).map(ToString::to_string),
                Some(camera.to_string())
            );
        }
    }

    #[test]
    fn inquiry_correlation_lost_is_unsent_live_session_and_not_retryable() {
        let error = Error::inquiry_correlation_lost(crate::CameraId::CAMERA_2);
        assert!(matches!(
            error,
            Error::InquiryCorrelationLost { camera } if camera == crate::CameraId::CAMERA_2
        ));
        assert_eq!(error.kind(), ErrorKind::NotExecutable);
        assert!(!error.is_retryable());
        assert_eq!(error.suggested_retry_delay(), None);
        assert!(!error.requires_new_session());
        assert_eq!(
            error.failure_context(),
            Some(FailureContext::new(
                FailureStage::Terminal,
                Certainty::NotAccepted
            ))
        );
        assert!(error.to_string().contains("Camera 2"), "{error}");
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
            Error::io_timeout(),
            Error::CommandBufferFull,
            Error::CommandPending,
            Error::RuntimeQueueFull { capacity: 8 },
            Error::CommandCanceled,
            Error::CommandNotExecutable,
            Error::SyntaxError,
            Error::NoSocket,
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
        let error = Error::CommandPending;
        let contextual = error.with_context("Operation failed");
        assert_eq!(contextual.kind(), ErrorKind::Busy);

        let error = Error::io_timeout();
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
            (Error::CommandPending, Duration::from_millis(50)),
            (Error::io_timeout(), Duration::from_secs(2)),
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

    /// D20 (#783): a caller's expired wait is its own variant, names the
    /// still-running request, and is never retryable.
    #[cfg(any(feature = "async", feature = "blocking"))]
    #[test]
    fn observation_timeout_is_a_live_request_that_must_not_be_resubmitted() {
        let operation = OperationId::from_raw(7);
        let error = Error::ObservationTimeout { operation };
        assert_eq!(error.kind(), ErrorKind::Timeout);
        assert!(!error.is_retryable());
        assert_eq!(error.suggested_retry_delay(), None);
        assert!(!error.requires_new_session());
        assert_eq!(
            error.failure_context(),
            Some(FailureContext::new(
                FailureStage::Observation,
                Certainty::StillLive
            ))
        );
        assert_eq!(
            error.to_string(),
            "Wait for operation 7 timed out; it is still running"
        );
        assert!(error.is_caller_deadline());
    }

    /// D20 (#783): every timeout carries its stage and certainty, and a
    /// context wrapper is looked through.
    #[test]
    fn every_timeout_constructor_carries_its_context() {
        let cases = [
            (
                Error::admission_timeout(),
                FailureStage::PreAdmission,
                Certainty::NotAccepted,
            ),
            (
                Error::query_timeout(),
                FailureStage::Observation,
                Certainty::NotAccepted,
            ),
            (
                Error::io_timeout(),
                FailureStage::Session,
                Certainty::Unconfirmed,
            ),
        ];
        for (error, stage, certainty) in cases {
            let expected = Some(FailureContext::new(stage, certainty));
            assert_eq!(error.failure_context(), expected, "{error:?}");
            assert_eq!(error.kind(), ErrorKind::Timeout);
            assert!(error.is_retryable(), "{error:?}");
            assert_eq!(error.clone().context("wrapped").failure_context(), expected);
        }
    }

    /// D20 (#783): a connection that could not be established submitted
    /// nothing.
    #[cfg(any(
        feature = "blocking",
        feature = "runtime-tokio",
        feature = "runtime-smol",
        feature = "transport-serial",
        feature = "transport-serial-tokio"
    ))]
    #[test]
    fn a_connect_timeout_submitted_nothing() {
        assert_eq!(
            Error::connect_timeout().failure_context(),
            Some(FailureContext::new(
                FailureStage::Session,
                Certainty::NotAccepted
            ))
        );
    }

    /// D20 (#783): the unconfirmed outcomes report where certainty was lost;
    /// other errors carry no failure context.
    #[test]
    fn unconfirmed_outcomes_carry_context_and_other_errors_do_not() {
        assert_eq!(
            Error::UnsequencedCommandUnconfirmed.failure_context(),
            Some(FailureContext::new(
                FailureStage::Terminal,
                Certainty::Unconfirmed
            ))
        );
        assert_eq!(
            Error::CancellationUnconfirmed.failure_context(),
            Some(FailureContext::new(
                FailureStage::CancellationAttempt,
                Certainty::Unconfirmed
            ))
        );
        for error in [
            Error::CommandBufferFull,
            Error::RuntimeShutdown,
            Error::SyntaxError,
        ] {
            assert_eq!(error.failure_context(), None, "{error:?}");
        }
    }

    /// D20 (#783): only a caller's own deadline is normalized into a query's
    /// repeatable timeout; a terminal protocol timeout keeps its context.
    #[cfg(any(feature = "async", feature = "blocking"))]
    #[test]
    fn query_normalization_keeps_protocol_timeouts_exact() {
        let observed = Error::ObservationTimeout {
            operation: OperationId::from_raw(3),
        };
        assert_eq!(
            observed.into_query_timeout().failure_context(),
            Error::query_timeout().failure_context()
        );
        assert_eq!(
            Error::admission_timeout()
                .into_query_timeout()
                .failure_context(),
            Error::query_timeout().failure_context()
        );
        let terminal = Error::timeout(FailureStage::Terminal, Certainty::FailedConclusively);
        assert_eq!(
            terminal.clone().into_query_timeout().failure_context(),
            terminal.failure_context()
        );
        assert!(!terminal.is_caller_deadline());
    }
}
