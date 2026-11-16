//! Error classification and recovery decisions.
//!
//! [`Error::recovery`] combines session validity, request certainty and the
//! temporary condition into one typed verdict. [`Error::kind`] and
//! [`Error::failure_context`] remain reporting facts. Only [`crate::error::Recovery::Resubmit`]
//! permits automatically submitting the request again. An unanswered heartbeat
//! remains an application policy decision: compare
//! [`crate::observability::MetricsSnapshot::received_frames`](crate::observability::MetricsSnapshot::received_frames)
//! around bounded activity and impose a silence threshold.
//!
//! ## Checked recovery matrix
//!
//! <!-- BEGIN GENERATED RECOVERY VERDICTS -->
//! | Error / condition | Kind | Stage / certainty | Recovery |
//! | --- | --- | --- | --- |
//! | `ConnectionFailed` | `IoRefused` | — | `Abandon` |
//! | `ConnectionClosed` | `IoClosed` | — | `ReopenSession` |
//! | `CommandPending / ACK received` | `Busy` | — | `WaitAgain` |
//! | `InvalidResponse` | `Protocol` | — | `Abandon` |
//! | `FeatureNotSupported` | `Unsupported` | — | `Abandon` |
//! | `Io / other OS error` | `Other` | — | `Abandon` |
//! | `SyntaxError` | `InvalidParameter` | — | `Abandon` |
//! | `CommandBufferFull` | `BufferFull` | — | `Resubmit after 200ms` |
//! | `CommandCancelled` | `Cancelled` | — | `Abandon` |
//! | `NoSocket` | `BufferFull` | — | `Resubmit after 200ms` |
//! | `CommandNotExecutable` | `NotExecutable` | — | `Abandon` |
//! | `InvalidResponseFormat` | `Protocol` | — | `Abandon` |
//! | `InvalidResponseLength` | `Protocol` | — | `Abandon` |
//! | `UnexpectedResponseType` | `Protocol` | — | `Abandon` |
//! | `Unknown` | `Protocol` | — | `Abandon` |
//! | `InvalidRequest` | `InvalidParameter` | — | `Abandon` |
//! | `MessageLengthError` | `Protocol` | — | `Abandon` |
//! | `ParseError` | `Protocol` | — | `Abandon` |
//! | `TransportError / no uncertain effect` | `Transport` | — | `Resubmit after 50ms` |
//! | `TransmissionNotSent / first attempt, invalid request` | `InvalidParameter` | Terminal / NotAccepted | `Abandon` |
//! | `TransmissionNotSent / earlier attempt uncertain` | `InvalidParameter` | Terminal / Unconfirmed | `Reconcile` |
//! | `InquiryFailed / timeout cause` | `Timeout` | Terminal / FailedConclusively | `Resubmit after 2s` |
//! | `InquiryFailed / command-scoped source` | `Unconfirmed` | Terminal / FailedConclusively | `Abandon` |
//! | `InquiryFailed / still-live source` | `Timeout` | Terminal / FailedConclusively | `Resubmit after 2s` |
//! | `InquiryFailed / session ended` | `IoClosed` | Terminal / FailedConclusively | `ReopenSession` |
//! | `InvalidParameter` | `InvalidParameter` | — | `Abandon` |
//! | `BufferTooSmall` | `InvalidParameter` | — | `Abandon` |
//! | `ParameterOutOfRange` | `InvalidParameter` | — | `Abandon` |
//! | `Timeout / terminal, not accepted` | `Timeout` | Terminal / NotAccepted | `Resubmit after 2s` |
//! | `MotionSuperseded / never sent` | `Cancelled` | Terminal / NotAccepted | `Abandon` |
//! | `SettlementSuperseded / deliberate observation end` | `Cancelled` | — | `Abandon` |
//! | `SettlementObservationFailed / live session` | `InvalidParameter` | Observation / Unconfirmed | `Reconcile` |
//! | `CancellationObservationFailed / operation still live` | `Timeout` | CancellationAttempt / StillLive | `WaitAgain` |
//! | `ObservationTimeout` | `Timeout` | Observation / StillLive | `WaitAgain` |
//! | `ControlReserveExhausted` | `BufferFull` | — | `Resubmit after 200ms` |
//! | `InquiryShapeCapacityExhausted` | `BufferFull` | PreAdmission / NotAccepted | `Abandon` |
//! | `MaxRetriesExceeded` | `Timeout` | — | `Abandon` |
//! | `CancellationNotSupported` | `Unsupported` | — | `Abandon` |
//! | `InvalidState` | `NotExecutable` | — | `Abandon` |
//! | `OwnerTerminated` | `IoClosed` | — | `ReopenSession` |
//! | `DiagnosticSubscriberLimit` | `Other` | — | `Abandon` |
//! | `RuntimeShutdown / deliberate` | `IoClosed` | — | `Abandon` |
//! | `CancellationUnconfirmed` | `Unconfirmed` | CancellationAttempt / Unconfirmed | `Reconcile` |
//! | `UnsequencedCommandUnconfirmed` | `Unconfirmed` | Terminal / Unconfirmed | `Reconcile` |
//! | `InquiryCorrelationLost / lane latched` | `NotExecutable` | Terminal / NotAccepted | `Abandon` |
//! | `CommandCorrelationLost / lane latched` | `NotExecutable` | Terminal / NotAccepted | `Abandon` |
//! | `CommandFailedAfterAck` | `Unconfirmed` | Terminal / Unconfirmed | `Reconcile` |
//! | `RequestUnconfirmed / later rejection after a lost ACK` | `Unconfirmed` | Terminal / Unconfirmed | `Reconcile` |
//! | `RuntimeIdentityExhausted` | `Other` | — | `Abandon` |
//! | `RuntimeQueueFull` | `BufferFull` | — | `Resubmit after 200ms` |
//! | `StreamPoisoned` | `IoClosed` | — | `ReopenSession` |
//! | `InvalidCameraId` | `InvalidParameter` | — | `Abandon` |
//! | `ResponseTooLarge` | `Protocol` | — | `Abandon` |
//! | `InquiryNotCancellable` | `InvalidParameter` | — | `Abandon` |
//! | `InvalidAddress` | `InvalidParameter` | — | `Abandon` |
//! | `UnsupportedTransport` | `Unsupported` | — | `Abandon` |
//! | `MissingRuntime` | `Unsupported` | — | `Abandon` |
//! | `WithContext / pending command` | `Busy` | — | `WaitAgain` |
//! | `Timeout / inquiry reply` | `Timeout` | Terminal / FailedConclusively | `Resubmit after 2s` |
//! | `Timeout / command outcome unknown` | `Timeout` | Terminal / Unconfirmed | `Reconcile` |
//! | `Timeout / write outcome unknown` | `Timeout` | Session / Unconfirmed | `Reconcile` |
//! | `Timeout / cancel attempt deadline` | `Timeout` | CancellationAttempt / StillLive | `WaitAgain` |
//! | `TransportError / uncertain write` | `Transport` | Session / Unconfirmed | `Reconcile` |
//! | `MotionSuperseded / earlier attempt uncertain` | `Cancelled` | Terminal / Unconfirmed | `Reconcile` |
//! <!-- END GENERATED RECOVERY VERDICTS -->

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
/// use grafton_visca::{error::ErrorKind, Error};
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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case")
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
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
/// [`Error::recovery`] answers whether to resubmit, wait on the existing
/// operation, reconcile state, reopen the session, or abandon automatic recovery.
/// [`Error::kind`] and [`Error::failure_context`] describe the underlying facts;
/// callers do not need to combine them into another replay policy.
///
/// ## Terminal Session Failures
/// A third category ends the session outright: the peer closed the connection,
/// the byte-stream position became unknowable, the owner terminated
/// unexpectedly, or the application shut the runtime down. A fatal receive closure is normalized to
/// [`Error::ConnectionClosed`] (with the transport cause retained in its
/// source); [`Error::StreamPoisoned`] is reserved for an unknowable stream
/// framing or write position. Use [`Error::requires_new_session()`] to tell
/// transport death from deliberate shutdown. A per-request correlation failure
/// for an unsequenced command on a raw-VISCA envelope is different:
/// [`Error::UnsequencedCommandUnconfirmed`] has
/// [`crate::error::ErrorKind::Unconfirmed`] and does not by itself make the session unusable.
///
/// # VISCA Error Codes
///
/// The VISCA protocol defines specific error codes that are mapped to Error variants:
/// - `0x01` → `MessageLengthError` - Message length incorrect
/// - `0x02` → `SyntaxError` - Command format invalid
/// - `0x03` → `CommandBufferFull` - Camera busy (always retryable)
/// - `0x04` → `CommandCancelled` - Command was canceled
/// - `0x05` → `NoSocket` - No socket available
/// - `0x41` → `CommandNotExecutable` - Command invalid in current state
///
/// Use [`Error::from_code()`] to convert VISCA error codes to Error variants.
///
/// # Example
///
/// One recipe applies to every error:
///
/// ```rust
/// use grafton_visca::{error::Recovery, Error};
///
/// fn next_step(error: &Error) -> &'static str {
///     match error.recovery() {
///         Recovery::Resubmit { .. } => "submit again after the specified delay",
///         Recovery::WaitAgain => "observe the existing operation",
///         Recovery::Reconcile => "inspect camera state before deciding",
///         Recovery::ReopenSession => "open a new session",
///         Recovery::Abandon => "do not retry automatically",
///         _ => "inspect this future recovery verdict",
///     }
/// }
///
/// assert_eq!(
///     next_step(&Error::CommandPending),
///     "observe the existing operation"
/// );
/// assert_eq!(
///     next_step(&Error::io_timeout()),
///     "inspect camera state before deciding"
/// );
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
    /// the underlying transport cause in its `source()`. A stream framing or write
    /// failure whose byte position is unknowable is reported as
    /// [`Self::StreamPoisoned`] instead.
    #[error("Connection closed{}", source.as_ref().map(|cause| format!(": {cause}")).unwrap_or_default())]
    #[non_exhaustive]
    ConnectionClosed {
        /// Original typed cause of the connection closure, when one was reported.
        #[source]
        source: Option<Arc<Error>>,
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
    #[error("IO error: {source}")]
    Io {
        /// The original operating-system error.
        #[source]
        source: Arc<io::Error>,
    },

    /// VISCA protocol syntax error (0x02): Command format is incorrect or parameters are illegal.
    #[error("Syntax error in VISCA command")]
    SyntaxError,

    /// VISCA protocol command buffer full error (0x03): Two sockets are already in use.
    #[error("Command buffer is full")]
    CommandBufferFull,

    /// VISCA protocol command canceled (0x04): Command was canceled in the specified socket.
    #[error("Command was canceled")]
    CommandCancelled,

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
    #[error("Unknown error code: {code:#04X}")]
    Unknown {
        /// The camera's unrecognized error code.
        code: u8,
    },

    /// Invalid request to socket manager.
    #[error("Invalid request: {reason}")]
    InvalidRequest {
        /// Why the request cannot be admitted or encoded.
        reason: Cow<'static, str>,
    },

    /// Message length error (0x01): Message length is incorrect.
    #[error("Message length error")]
    MessageLengthError,

    /// Failed to parse response data.
    #[error("Parse error: {reason}")]
    ParseError {
        /// Why the response could not be parsed.
        reason: Cow<'static, str>,
    },

    /// One transport operation failed while the session remains usable.
    ///
    /// This is an operation-scoped failure with [`crate::error::ErrorKind::Transport`].
    /// Recovery also respects any request certainty in its source. The receive side remains authoritative for
    /// session death; use [`Self::requires_new_session()`] rather than treating
    /// this error as a reconnect signal. The original cause is retained through
    /// [`std::error::Error::source`], including any OS error and request certainty.
    #[error("Transport error: {source}")]
    #[non_exhaustive]
    TransportError {
        /// The original failure, including its OS cause and request certainty.
        #[source]
        source: Arc<Error>,
    },

    /// This transmission attempt sent no bytes; an earlier attempt may have.
    ///
    /// The owner refused framing or write preparation before transport I/O.
    /// `context` records the logical request's certainty across all attempts,
    /// so a prior possible effect remains `Unconfirmed`. This attempt does not
    /// end the session, and the original preparation failure remains its source.
    #[error("Transmission was not sent: {source}")]
    #[non_exhaustive]
    TransmissionNotSent {
        /// The original framing or preparation failure.
        #[source]
        source: Arc<Error>,
        /// Certainty of the logical request, including any earlier attempt.
        context: FailureContext,
    },

    /// A failed inquiry terminal outcome, preserving its original cause.
    ///
    /// Inquiries change no camera state, so their terminal failure has fixed
    /// `Terminal` / `FailedConclusively` certainty even when a custom transport
    /// reports a command-scoped or still-live cause. Caller observation timeouts,
    /// preparation refusals and standalone decoder errors are not wrapped.
    #[error("Inquiry failed: {source}")]
    #[non_exhaustive]
    InquiryFailed {
        /// The original terminal failure.
        #[source]
        source: Arc<Error>,
    },

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
    /// `context` says which deadline (the [`crate::error::FailureStage`]) and what is known
    /// about the request's effect (the [`crate::error::Certainty`]): an admission deadline
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
    /// observe the existing operation. [`Self::recovery`] returns [`crate::error::Recovery::WaitAgain`].
    #[error("Wait for operation {operation} timed out; it is still running")]
    #[non_exhaustive]
    ObservationTimeout {
        /// The admitted request the wait observed.
        operation: OperationId,
    },

    /// An owner halt superseded older declared motion before it finished.
    ///
    /// `context` reports what is known about the superseded request. Its
    /// certainty is [`crate::error::Certainty::NotAccepted`] when no attempt was ever
    /// written, or when every written attempt was conclusively rejected by
    /// the camera. Deliberate supersession still returns [`crate::error::Recovery::Abandon`],
    /// so an automatic loop must not undo the halt by resubmitting. It is
    /// [`crate::error::Certainty::Unconfirmed`] when an earlier attempt may have reached
    /// the camera; reconcile before resubmitting. The stage is
    /// [`crate::error::FailureStage::PreAdmission`] when the fence rejected the submission
    /// itself and [`crate::error::FailureStage::Terminal`] when it ended an admitted
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

    /// A recorded cancellation attempt failed while the operation remains live.
    /// A repeated `cancel()` observes
    /// this cached answer and cannot send another cancellation for the intent.
    /// The underlying condition remains available as the error source; the
    /// operation's own outcome must still be observed separately.
    #[error("Could not resolve the recorded cancellation intent: {source}")]
    #[non_exhaustive]
    CancellationObservationFailed {
        /// The exact cancellation write or observation failure.
        #[source]
        source: Box<Error>,
    },

    /// Maximum retry attempts exceeded.
    #[error("Maximum retries exceeded")]
    MaxRetriesExceeded,

    /// Operation is not supported by this implementation.
    #[error("Operation not supported")]
    CancellationNotSupported,

    /// Operation cannot be performed in current state.
    #[error("Invalid state: {reason}")]
    InvalidState {
        /// Why the operation cannot run in the current state.
        reason: Cow<'static, str>,
    },

    /// Runtime has been shutdown.
    #[error("Runtime has been shutdown")]
    RuntimeShutdown,

    /// The owner stopped without publishing a terminal result, for example
    /// because its worker or task panicked. The session cannot make progress
    /// and must be replaced.
    #[error("Session owner terminated without publishing a terminal result")]
    OwnerTerminated,

    /// The session already has its maximum number of diagnostic subscribers.
    /// Dropping a subscription makes room; request-admission capacity does not
    /// affect this limit.
    #[error("Diagnostic subscriber limit reached ({capacity} subscribers)")]
    #[non_exhaustive]
    DiagnosticSubscriberLimit {
        /// Maximum simultaneous diagnostic subscriptions for this session.
        capacity: usize,
    },

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
    /// [`crate::config::SessionConfig::with_strict_unconfirmed_poison`] opt-in instead poisons
    /// the whole session, which is surfaced as [`Self::StreamPoisoned`].
    ///
    /// [`crate::config::SessionConfig::with_strict_unconfirmed_poison`]: crate::config::SessionConfig::with_strict_unconfirmed_poison
    #[error("Unsequenced command outcome could not be confirmed")]
    UnsequencedCommandUnconfirmed,

    /// Raw-VISCA inquiries to `camera` cannot be correlated on this session,
    /// so this request was not sent.
    ///
    /// On a stream transport (TCP, serial) an inquiry that timed out after it
    /// was written is still owed its reply: the stream loses nothing, and raw
    /// inquiry replies carry no identity, so that late reply would otherwise be
    /// returned as the next inquiry's data. The session waits the profile's
    /// [`ambiguity_timeout`](crate::profiles::ProfileTiming::ambiguity_timeout)
    /// for it. If it has not arrived by then — a long stall, a camera that
    /// never answers that inquiry, or an absent address — every inquiry to
    /// that camera (and every user-declared
    /// [`CompletionOnly`](crate::raw::RawReplyShape::CompletionOnly) raw
    /// command, whose completion is equally unkeyed) fails with this error
    /// without being written, until the owed reply arrives or a later answer
    /// proves it never will.
    ///
    /// Nothing was sent, so the failure context is `Terminal` /
    /// [`crate::error::Certainty::NotAccepted`]. It is not retryable on this session: retrying
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
    /// [`ambiguity_timeout`](crate::profiles::ProfileTiming::ambiguity_timeout).
    /// If it has not arrived by then, every ordinary ACK-bearing and
    /// `CompletionOnly` command to that camera fails with this error without
    /// being written, until it arrives or a later answer from the camera (the
    /// reply to a later inquiry, or the answer to a later STOP) proves it
    /// never will; a `CompletionOnly` command's completion, which comes only
    /// once it has run, is settled only by itself.
    ///
    /// Nothing was sent, so the failure context is `Terminal` /
    /// [`crate::error::Certainty::NotAccepted`]. It is not retryable on this session until
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
    /// reports `Terminal` / [`crate::error::Certainty::Unconfirmed`] whatever the camera's
    /// code would mean before an ACK. Reconcile the camera state before
    /// resubmitting.
    #[error("Camera reported an error after acknowledging the command: {source}")]
    #[non_exhaustive]
    CommandFailedAfterAck {
        /// The camera's error, exactly as it would be reported before an ACK.
        #[source]
        source: Box<Error>,
    },

    /// A request failed after a written attempt may already have taken effect.
    ///
    /// A later rejection or transport failure cannot erase that possibility.
    /// The exact terminal cause is retained as `source`, but recovery always
    /// requires reconciling camera state before resubmission (#962).
    #[error("Request outcome could not be confirmed: {source}")]
    #[non_exhaustive]
    RequestUnconfirmed {
        /// The exact camera or transport error that ended the request.
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
    /// The admission capacity is configurable via [`crate::config::SessionConfig::admission_capacity`].
    ///
    /// [`crate::config::SessionConfig::admission_capacity`]: crate::config::SessionConfig::admission_capacity
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

    /// A new raw inquiry payload-shape declaration could not be admitted.
    ///
    /// The target's bounded declaration set is already reserved by admitted
    /// work or retained correlation evidence. This request was not accepted
    /// and sent no bytes; the session remains usable. Recovery is
    /// [`crate::error::Recovery::Abandon`], rather than an automatic capacity retry.
    #[error("Raw inquiry payload-shape capacity exhausted for camera {camera} (maximum {capacity} tracked declarations)")]
    #[non_exhaustive]
    InquiryShapeCapacityExhausted {
        /// The individual camera whose declaration set is full.
        camera: crate::CameraId,
        /// The maximum number of tracked declarations for that target.
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
    #[error("Stream transport poisoned: {source}")]
    #[non_exhaustive]
    StreamPoisoned {
        /// Original typed framing, write, or protocol cause.
        #[source]
        source: Arc<Error>,
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
    InquiryNotCancellable,

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
    /// Wraps another error while preserving its typed recovery verdict and failure facts.
    #[error("{context}: {source}")]
    #[non_exhaustive]
    WithContext {
        /// Human-readable context describing what operation failed.
        context: Cow<'static, str>,
        /// The underlying error that occurred.
        source: Box<Error>,
    },
}

/// The complete recovery decision for one failed request or observation.
///
/// Only `Resubmit` permits a new submission. The other verdicts retain the
/// distinction between a live operation, an uncertain effect, a dead session,
/// and a deliberate or permanent failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case", deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum Recovery {
    /// The request had no effect and a temporary condition allows another attempt.
    Resubmit {
        /// Wait this long before submitting it again.
        #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number; nanos: number }"))]
        after: Duration,
    },
    /// The original request is still live; observe its eventual outcome.
    WaitAgain,
    /// The request may have taken effect; inspect camera state before deciding.
    Reconcile,
    /// The session is unusable; open a new one.
    ReopenSession,
    /// Do not retry automatically: the condition is permanent or deliberate.
    Abandon,
}

/// Recovery for an effect-free terminal failure, after session validity is known.
fn conclusive_recovery(kind: ErrorKind) -> Recovery {
    match kind {
        ErrorKind::Timeout => Recovery::Resubmit {
            after: Duration::from_secs(2),
        },
        ErrorKind::BufferFull => Recovery::Resubmit {
            after: Duration::from_millis(200),
        },
        ErrorKind::Busy | ErrorKind::Transport => Recovery::Resubmit {
            after: Duration::from_millis(50),
        },
        ErrorKind::IoClosed
        | ErrorKind::IoRefused
        | ErrorKind::Unconfirmed
        | ErrorKind::Cancelled
        | ErrorKind::NotExecutable
        | ErrorKind::Protocol
        | ErrorKind::Unsupported
        | ErrorKind::InvalidParameter
        | ErrorKind::Other => Recovery::Abandon,
    }
}

/// Where a deadline or certainty failure happened, and what is known about
/// the affected request's effect. Read it with [`Error::failure_context`].
///
/// Recovery decisions come from [`Error::recovery`]; certainty alone does not
/// authorize replay of deliberate supersession or a latched correlation lane.
///
/// # Example
///
/// ```rust
/// use grafton_visca::{error::Recovery, Error};
///
/// fn next_step(error: &Error) -> &'static str {
///     match error.recovery() {
///         Recovery::Resubmit { .. } => "submit again after the specified delay",
///         Recovery::WaitAgain => "observe the existing operation",
///         Recovery::Reconcile => "inspect camera state before deciding",
///         Recovery::ReopenSession => "open a new session",
///         Recovery::Abandon => "do not retry automatically",
///         _ => "inspect this future recovery verdict",
///     }
/// }
///
/// assert_eq!(
///     next_step(&Error::CommandPending),
///     "observe the existing operation"
/// );
/// assert_eq!(
///     next_step(&Error::io_timeout()),
///     "inspect camera state before deciding"
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case")
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case")
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
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

    /// Reports a closed connection with an optional typed cause.
    ///
    /// An existing close verdict, including one beneath context wrappers, is
    /// returned unchanged instead of adding another session-death wrapper.
    #[must_use]
    pub fn connection_closed(source: Option<Arc<Error>>) -> Self {
        match source {
            Some(source) if matches!(source.without_context(), Self::ConnectionClosed { .. }) => {
                source.as_ref().clone()
            }
            source => Self::ConnectionClosed { source },
        }
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

    /// Records a failed cancellation attempt while its original request lives.
    /// Canonical operation/session verdicts remain exact; a transport or timeout
    /// cause is kept inside a fixed CancellationAttempt/StillLive wrapper.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn cancellation_observation_failed(source: Error) -> Self {
        fn attempt_failure(error: &Error) -> bool {
            match error {
                Error::WithContext { source, .. } => attempt_failure(source),
                Error::Timeout { context } => context.certainty != Certainty::StillLive,
                Error::TransportError { .. } => error.recovery() != Recovery::WaitAgain,
                _ => matches!(error.recovery(), Recovery::Resubmit { .. }),
            }
        }
        if !attempt_failure(&source) {
            return source;
        }
        Self::CancellationObservationFailed {
            source: Box::new(source),
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

    /// Reports a terminal cause after an attempt may have taken effect.
    #[must_use]
    pub fn request_unconfirmed(source: impl Into<Box<Error>>) -> Self {
        Self::RequestUnconfirmed {
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
    ///
    /// An existing poison verdict, including one beneath context wrappers, is
    /// returned unchanged, preserving its typed cause.
    #[must_use]
    pub fn stream_poisoned(source: impl Into<Arc<Error>>) -> Self {
        let source = source.into();
        if matches!(source.without_context(), Self::StreamPoisoned { .. }) {
            source.as_ref().clone()
        } else {
            Self::StreamPoisoned { source }
        }
    }

    /// Context is descriptive and does not change a terminal cause's identity.
    fn without_context(&self) -> &Self {
        let mut error = self;
        while let Self::WithContext { source, .. } = error {
            error = source;
        }
        error
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
    /// use grafton_visca::{error::ErrorKind, Error};
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
            Self::CommandCancelled => ErrorKind::Cancelled,

            // BufferFull: capacity exhaustion; recovery distinguishes its conditions.
            Self::CommandBufferFull
            | Self::RuntimeQueueFull { .. }
            | Self::ControlReserveExhausted { .. }
            | Self::InquiryShapeCapacityExhausted { .. }
            | Self::NoSocket => ErrorKind::BufferFull,

            // NotExecutable: command invalid in current state
            Self::CommandNotExecutable
            | Self::InvalidState { .. }
            | Self::InquiryCorrelationLost { .. }
            | Self::CommandCorrelationLost { .. } => ErrorKind::NotExecutable,

            // IoClosed: connection/transport no longer usable
            Self::ConnectionClosed { .. }
            | Self::RuntimeShutdown
            | Self::StreamPoisoned { .. }
            | Self::OwnerTerminated => ErrorKind::IoClosed,

            // Transport: one failed operation on a live session.
            Self::TransportError { .. } => ErrorKind::Transport,
            Self::TransmissionNotSent { source, .. } | Self::InquiryFailed { source, .. } => {
                source.kind()
            }

            // Unconfirmed: one transmitted operation has an unknowable result
            Self::CancellationUnconfirmed
            | Self::UnsequencedCommandUnconfirmed
            | Self::CommandFailedAfterAck { .. }
            | Self::RequestUnconfirmed { .. } => ErrorKind::Unconfirmed,

            // IoRefused: connection attempt rejected
            Self::ConnectionFailed { .. } => ErrorKind::IoRefused,

            // Protocol: malformed or unexpected wire data
            Self::InvalidResponseFormat
            | Self::InvalidResponseLength { .. }
            | Self::UnexpectedResponseType
            | Self::ParseError { .. }
            | Self::MessageLengthError
            | Self::InvalidResponse { .. }
            | Self::Unknown { .. }
            | Self::ResponseTooLarge { .. } => ErrorKind::Protocol,

            // Unsupported: feature/capability not available
            Self::FeatureNotSupported { .. }
            | Self::CancellationNotSupported
            | Self::MissingRuntime => ErrorKind::Unsupported,

            // InvalidParameter: caller-supplied value is invalid
            Self::InvalidParameter { .. }
            | Self::ParameterOutOfRange { .. }
            | Self::SyntaxError
            | Self::InvalidRequest { .. }
            | Self::BufferTooSmall { .. }
            | Self::InvalidCameraId { .. }
            | Self::InquiryNotCancellable { .. }
            | Self::InvalidAddress { .. } => ErrorKind::InvalidParameter,
            Self::UnsupportedTransport { .. } => ErrorKind::Unsupported,

            // Busy: transient contention
            Self::CommandPending => ErrorKind::Busy,

            // Other: truly uncategorizable
            Self::RuntimeIdentityExhausted | Self::DiagnosticSubscriberLimit { .. } => {
                ErrorKind::Other
            }

            // Delegated: unwrap context wrapper
            Self::WithContext { source, .. }
            | Self::SettlementObservationFailed { source, .. }
            | Self::CancellationObservationFailed { source, .. } => source.kind(),
            Self::MotionSuperseded { .. } | Self::SettlementSuperseded { .. } => {
                ErrorKind::Cancelled
            }

            // Io: inspect inner io::ErrorKind
            Self::Io { source: arc } => match arc.kind() {
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
    /// Transport close, owner termination, explicit shutdown, and poison have
    /// distinct terminal errors, but they all share [`crate::error::ErrorKind::IoClosed`]. Matching on
    /// the kind therefore cannot separate "the camera dropped the connection"
    /// from "this application asked the runtime to stop". This predicate draws
    /// exactly that line without exposing implementation details:
    ///
    /// - `true` — the session lost its transport or owner: the peer closed
    ///   the connection ([`Error::ConnectionClosed`]), the byte-stream
    ///   position became unknowable ([`Error::StreamPoisoned`]), or the owner
    ///   stopped without publishing a terminal result ([`Error::OwnerTerminated`]). A poisoned
    ///   session is terminal and is never revived; every retained and
    ///   subsequently attempted operation keeps reporting its exact terminal
    ///   session error. Fatal receive closure is always normalized to
    ///   `ConnectionClosed`, with the underlying cause in its source chain. An
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
    /// Keep the reusable [`crate::config::SessionConfig`], build a fresh transport, and open a
    /// new session; the old session, camera views, operation handles, and
    /// subscriptions cannot be rebound to it. The new session's state cache
    /// starts `Unknown` and nothing is resubmitted automatically, so re-query
    /// supported camera state before applying desired state. Do not blindly
    /// replay a command whose completion is uncertain: it may have reached the
    /// camera before the connection failed or unsequenced raw-VISCA
    /// correlation was poisoned.
    ///
    /// [`crate::config::SessionConfig`]: crate::config::SessionConfig
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
            Self::ConnectionClosed { .. } | Self::StreamPoisoned { .. } | Self::OwnerTerminated => {
                true
            }

            // Deliberate shutdown. The session is over because the application
            // ended it, so reconnecting is a policy decision, not a repair.
            Self::RuntimeShutdown => false,

            // Delegated: an added context string never changes the condition.
            Self::WithContext { source, .. }
            | Self::SettlementObservationFailed { source, .. }
            | Self::CancellationObservationFailed { source, .. } => source.requires_new_session(),

            Self::InquiryFailed { source, .. } => source.requires_new_session(),

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
            | Self::Io { .. }
            | Self::SyntaxError
            | Self::CommandBufferFull
            | Self::CommandCancelled
            | Self::NoSocket
            | Self::CommandNotExecutable
            | Self::InvalidResponseFormat
            | Self::InvalidResponseLength { .. }
            | Self::UnexpectedResponseType
            | Self::Unknown { .. }
            | Self::InvalidRequest { .. }
            | Self::MessageLengthError
            | Self::ParseError { .. }
            | Self::TransportError { .. }
            | Self::TransmissionNotSent { .. }
            | Self::InvalidParameter { .. }
            | Self::BufferTooSmall { .. }
            | Self::ParameterOutOfRange { .. }
            | Self::Timeout { .. }
            | Self::ObservationTimeout { .. }
            | Self::MotionSuperseded { .. }
            | Self::SettlementSuperseded { .. }
            | Self::MaxRetriesExceeded
            | Self::CancellationNotSupported
            | Self::InvalidState { .. }
            | Self::CancellationUnconfirmed
            | Self::UnsequencedCommandUnconfirmed
            | Self::InquiryCorrelationLost { .. }
            | Self::CommandCorrelationLost { .. }
            | Self::CommandFailedAfterAck { .. }
            | Self::RequestUnconfirmed { .. }
            | Self::RuntimeIdentityExhausted
            | Self::DiagnosticSubscriberLimit { .. }
            | Self::RuntimeQueueFull { .. }
            | Self::ControlReserveExhausted { .. }
            | Self::InquiryShapeCapacityExhausted { .. }
            | Self::InvalidCameraId { .. }
            | Self::ResponseTooLarge { .. }
            | Self::InquiryNotCancellable
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
            0x04 => Self::CommandCancelled,
            0x05 => Self::NoSocket,
            0x41 => Self::CommandNotExecutable, // Context-dependent retryability
            _ => Self::Unknown { code },
        }
    }

    /// Returns the complete recovery verdict for this failure.
    ///
    /// The verdict combines session validity, request certainty and whether the
    /// condition is temporary. Only [`crate::error::Recovery::Resubmit`] permits submitting
    /// the request again; its delay is part of that verdict. An acknowledged or
    /// unconfirmed command is never automatically replayed.
    #[must_use]
    pub fn recovery(&self) -> Recovery {
        const CAPACITY: Recovery = Recovery::Resubmit {
            after: Duration::from_millis(200),
        };
        const TIMEOUT: Recovery = Recovery::Resubmit {
            after: Duration::from_secs(2),
        };
        match self {
            Self::WithContext { source, .. } => source.recovery(),
            Self::ConnectionClosed { .. } | Self::StreamPoisoned { .. } | Self::OwnerTerminated => {
                Recovery::ReopenSession
            }
            Self::SettlementObservationFailed { source, .. } => {
                if source.requires_new_session() {
                    Recovery::ReopenSession
                } else {
                    Recovery::Reconcile
                }
            }
            Self::CancellationObservationFailed { source, .. } => {
                if source.requires_new_session() {
                    Recovery::ReopenSession
                } else {
                    Recovery::WaitAgain
                }
            }
            Self::CommandPending | Self::ObservationTimeout { .. } => Recovery::WaitAgain,
            Self::CancellationUnconfirmed
            | Self::UnsequencedCommandUnconfirmed
            | Self::CommandFailedAfterAck { .. }
            | Self::RequestUnconfirmed { .. } => Recovery::Reconcile,
            Self::Timeout { context } => match context.certainty {
                Certainty::StillLive => Recovery::WaitAgain,
                Certainty::Unconfirmed => Recovery::Reconcile,
                Certainty::NotAccepted | Certainty::FailedConclusively => TIMEOUT,
            },
            Self::InquiryFailed { source, .. } => {
                if source.requires_new_session() {
                    Recovery::ReopenSession
                } else {
                    conclusive_recovery(source.kind())
                }
            }
            Self::TransmissionNotSent { source, context } => match context.certainty {
                Certainty::Unconfirmed => Recovery::Reconcile,
                Certainty::StillLive => Recovery::WaitAgain,
                Certainty::NotAccepted | Certainty::FailedConclusively => {
                    conclusive_recovery(source.kind())
                }
            },
            Self::TransportError { source, .. } => {
                match source.failure_context().map(|context| context.certainty) {
                    Some(Certainty::StillLive) => Recovery::WaitAgain,
                    Some(Certainty::Unconfirmed) => Recovery::Reconcile,
                    Some(Certainty::NotAccepted | Certainty::FailedConclusively) | None => {
                        Recovery::Resubmit {
                            after: Duration::from_millis(50),
                        }
                    }
                }
            }
            Self::MotionSuperseded { context, .. } => match context.certainty {
                Certainty::StillLive => Recovery::WaitAgain,
                Certainty::Unconfirmed => Recovery::Reconcile,
                Certainty::NotAccepted | Certainty::FailedConclusively => Recovery::Abandon,
            },
            Self::CommandBufferFull
            | Self::NoSocket
            | Self::RuntimeQueueFull { .. }
            | Self::ControlReserveExhausted { .. } => CAPACITY,
            Self::Io { source } => match source.kind() {
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => TIMEOUT,
                _ => Recovery::Abandon,
            },
            Self::ConnectionFailed { .. }
            | Self::InvalidResponse { .. }
            | Self::FeatureNotSupported { .. }
            | Self::SyntaxError
            | Self::CommandCancelled
            | Self::CommandNotExecutable
            | Self::InvalidResponseFormat
            | Self::InvalidResponseLength { .. }
            | Self::UnexpectedResponseType
            | Self::Unknown { .. }
            | Self::InvalidRequest { .. }
            | Self::MessageLengthError
            | Self::ParseError { .. }
            | Self::InvalidParameter { .. }
            | Self::BufferTooSmall { .. }
            | Self::ParameterOutOfRange { .. }
            | Self::SettlementSuperseded { .. }
            | Self::MaxRetriesExceeded
            | Self::CancellationNotSupported
            | Self::InvalidState { .. }
            | Self::RuntimeShutdown
            | Self::DiagnosticSubscriberLimit { .. }
            | Self::InquiryShapeCapacityExhausted { .. }
            | Self::InquiryCorrelationLost { .. }
            | Self::CommandCorrelationLost { .. }
            | Self::RuntimeIdentityExhausted
            | Self::InvalidCameraId { .. }
            | Self::ResponseTooLarge { .. }
            | Self::InquiryNotCancellable
            | Self::InvalidAddress { .. }
            | Self::UnsupportedTransport { .. }
            | Self::MissingRuntime => Recovery::Abandon,
        }
    }

    /// Where a deadline or certainty failure happened, and what is known
    /// about the affected request's effect.
    ///
    /// Timeouts and lifecycle uncertainty carry a context. A failed cancellation
    /// attempt always describes the original operation as `CancellationAttempt` /
    /// `StillLive`, independently of its source. Context and transport wrappers
    /// retain their source's facts. Deliberate settlement supersession has no new
    /// effect uncertainty and returns `None`.
    ///
    /// # Example
    /// ```rust
    /// use grafton_visca::{error::Certainty, error::FailureStage, error::Recovery, Error};
    /// let error = Error::io_timeout();
    /// assert_eq!(error.recovery(), Recovery::Reconcile);
    /// assert_eq!(
    ///     error
    ///         .failure_context()
    ///         .map(|facts| (facts.stage, facts.certainty)),
    ///     Some((FailureStage::Session, Certainty::Unconfirmed))
    /// );
    /// ```
    #[must_use]
    pub fn failure_context(&self) -> Option<FailureContext> {
        match self {
            Self::Timeout { context } | Self::TransmissionNotSent { context, .. } => Some(*context),
            Self::InquiryShapeCapacityExhausted { .. } => Some(FailureContext::new(
                FailureStage::PreAdmission,
                Certainty::NotAccepted,
            )),
            Self::InquiryFailed { .. } => Some(FailureContext::new(
                FailureStage::Terminal,
                Certainty::FailedConclusively,
            )),
            Self::SettlementObservationFailed { .. } => Some(FailureContext::new(
                FailureStage::Observation,
                Certainty::Unconfirmed,
            )),
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
            Self::CommandFailedAfterAck { .. } | Self::RequestUnconfirmed { .. } => Some(
                FailureContext::new(FailureStage::Terminal, Certainty::Unconfirmed),
            ),
            Self::CancellationObservationFailed { .. } => Some(FailureContext::new(
                FailureStage::CancellationAttempt,
                Certainty::StillLive,
            )),
            Self::WithContext { source, .. } => source.failure_context(),
            Self::TransportError { source, .. } => source.failure_context(),
            Self::ConnectionFailed { .. }
            | Self::ConnectionClosed { .. }
            | Self::CommandPending
            | Self::InvalidResponse { .. }
            | Self::FeatureNotSupported { .. }
            | Self::Io { .. }
            | Self::SyntaxError
            | Self::CommandBufferFull
            | Self::CommandCancelled
            | Self::NoSocket
            | Self::CommandNotExecutable
            | Self::InvalidResponseFormat
            | Self::InvalidResponseLength { .. }
            | Self::UnexpectedResponseType
            | Self::Unknown { .. }
            | Self::InvalidRequest { .. }
            | Self::MessageLengthError
            | Self::ParseError { .. }
            | Self::InvalidParameter { .. }
            | Self::BufferTooSmall { .. }
            | Self::ParameterOutOfRange { .. }
            | Self::SettlementSuperseded { .. }
            | Self::MaxRetriesExceeded
            | Self::CancellationNotSupported
            | Self::InvalidState { .. }
            | Self::RuntimeShutdown
            | Self::OwnerTerminated
            | Self::DiagnosticSubscriberLimit { .. }
            | Self::RuntimeIdentityExhausted
            | Self::RuntimeQueueFull { .. }
            | Self::ControlReserveExhausted { .. }
            | Self::StreamPoisoned { .. }
            | Self::InvalidCameraId { .. }
            | Self::ResponseTooLarge { .. }
            | Self::InquiryNotCancellable
            | Self::InvalidAddress { .. }
            | Self::UnsupportedTransport { .. }
            | Self::MissingRuntime => None,
        }
    }

    /// A pre-I/O refusal carries certainty for the whole logical request,
    /// including possibly effective earlier attempts.
    #[cfg(any(feature = "blocking", feature = "async"))]
    pub(crate) fn inquiry_shape_capacity_exhausted(
        camera: crate::CameraId,
        capacity: usize,
    ) -> Self {
        Self::InquiryShapeCapacityExhausted { camera, capacity }
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    pub(crate) fn transmission_not_sent(source: Error, certainty: Certainty) -> Self {
        Self::TransmissionNotSent {
            source: Arc::new(source),
            context: FailureContext::new(FailureStage::Terminal, certainty),
        }
    }

    /// Engine inquiry failures cannot leave an uncertain camera-state effect.
    #[cfg(any(feature = "blocking", feature = "async"))]
    pub(crate) fn inquiry_failed(source: Error) -> Self {
        match source {
            Self::InquiryFailed { .. } => source,
            source => Self::InquiryFailed {
                source: Arc::new(source),
            },
        }
    }

    /// Preserves the cause of a failed transport operation on a usable session.
    ///
    /// An existing transport-operation wrapper is returned unchanged; repeated
    /// normalization never adds another layer to its source chain.
    #[must_use]
    pub fn transport_error(source: impl Into<Error>) -> Self {
        let source = source.into();
        match source {
            Self::TransportError { .. } => source,
            source => Self::TransportError {
                source: Arc::new(source),
            },
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
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) const fn admission_timeout() -> Self {
        Self::timeout(FailureStage::PreAdmission, Certainty::NotAccepted)
    }

    /// A read-only state query (an idle wait or a motion query) ran out of
    /// time. It changed nothing, so it can be repeated.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) const fn query_timeout() -> Self {
        Self::timeout(FailureStage::Observation, Certainty::NotAccepted)
    }

    /// Whether a caller's own deadline produced this error: its wait expired,
    /// or its admission deadline passed. The request's protocol lifecycle did
    /// not.
    #[cfg(any(feature = "blocking", feature = "async"))]
    pub(crate) fn is_caller_deadline(&self) -> bool {
        match self {
            Self::WithContext { source, .. } => source.is_caller_deadline(),
            Self::ObservationTimeout { .. } => true,
            Self::Timeout { context } => {
                context.certainty == Certainty::NotAccepted
                    && matches!(
                        context.stage,
                        FailureStage::PreAdmission | FailureStage::Observation
                    )
            }
            _ => false,
        }
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
    /// not have left, so its certainty is [`crate::error::Certainty::Unconfirmed`].
    #[must_use]
    pub const fn io_timeout() -> Self {
        Self::timeout(FailureStage::Session, Certainty::Unconfirmed)
    }

    /// Add operation context to this error.
    ///
    /// Wraps the error with additional context information while preserving
    /// typed recovery verdict and failure facts.
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
    /// assert_eq!(
    ///     contextual_error.recovery(),
    ///     grafton_visca::error::Recovery::Resubmit {
    ///         after: std::time::Duration::from_millis(200),
    ///     }
    /// );
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
        Self::Io {
            source: Arc::new(err),
        }
    }
}

impl From<nom::Err<nom::error::Error<&[u8]>>> for Error {
    fn from(err: nom::Err<nom::error::Error<&[u8]>>) -> Self {
        Self::ParseError {
            reason: Cow::Owned(err.to_string()),
        }
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
    fn session_death_preserves_typed_sources_and_normalizes_context_idempotently() {
        let original = Arc::new(Error::from(io::Error::from_raw_os_error(104)));
        for closed in [true, false] {
            let terminal = if closed {
                Error::connection_closed(Some(original.clone()))
            } else {
                Error::stream_poisoned(original.clone())
            };
            let Some(cause) = std::error::Error::source(&terminal)
                .and_then(|source| source.downcast_ref::<Arc<Error>>())
            else {
                panic!("terminal verdict exposes its typed shared Error source");
            };
            assert!(
                matches!(cause.as_ref(), Error::Io { source: io } if io.raw_os_error() == Some(104))
            );
            let clone = terminal.clone();
            let retained = match &clone {
                Error::ConnectionClosed {
                    source: Some(source),
                }
                | Error::StreamPoisoned { source } => source,
                _ => panic!("terminal source"),
            };
            assert!(Arc::ptr_eq(retained, &original));
            for depth in 0..6 {
                let mut contextual = terminal.clone();
                for _ in 0..depth {
                    contextual = contextual.with_context("receive context");
                }
                let expected = contextual.to_string();
                let normalized = if closed {
                    Error::connection_closed(Some(Arc::new(contextual)))
                } else {
                    Error::stream_poisoned(contextual)
                };
                assert_eq!(normalized.to_string(), expected);
                assert_eq!(normalized.kind(), terminal.kind());
                assert_eq!(normalized.recovery(), terminal.recovery());
                assert_eq!(normalized.failure_context(), terminal.failure_context());
                assert!(normalized.requires_new_session());
                let mut cursor: &(dyn std::error::Error + 'static) = &normalized;
                let mut verdicts = 0;
                loop {
                    if matches!(
                        cursor
                            .downcast_ref::<Error>()
                            .or_else(|| cursor.downcast_ref::<Arc<Error>>().map(Arc::as_ref))
                            .or_else(|| cursor.downcast_ref::<Box<Error>>().map(Box::as_ref)),
                        Some(Error::ConnectionClosed { .. } | Error::StreamPoisoned { .. })
                    ) {
                        verdicts += 1;
                    }
                    let Some(source) = cursor.source() else { break };
                    cursor = source;
                }
                assert_eq!(verdicts, 1);
            }
        }
    }

    #[test]
    fn transport_wrapper_preserves_source_chain_and_is_idempotent() {
        let error = Error::transport_error(io::Error::from_raw_os_error(111));
        assert_eq!(error.kind(), ErrorKind::Transport);
        assert!(!error.requires_new_session());
        let Error::TransportError { source, .. } = &error else {
            panic!("transport wrapper")
        };
        let Error::Io { source: os } = source.as_ref() else {
            panic!("original OS source")
        };
        assert_eq!(os.raw_os_error(), Some(111));
        let Some(intermediate) = std::error::Error::source(&error) else {
            panic!("typed source")
        };
        let Some(original) = intermediate.source() else {
            panic!("OS source chain")
        };
        assert_eq!(original.to_string(), os.to_string());
        let again = Error::transport_error(error.clone());
        let Error::TransportError { source: same, .. } = again else {
            panic!("same wrapper")
        };
        assert!(Arc::ptr_eq(source, &same));
    }

    #[test]
    fn transport_wrapper_keeps_certainty_without_claiming_session_death() {
        let timeout = Error::io_timeout();
        let context = timeout.failure_context();
        assert_eq!(Error::transport_error(timeout).failure_context(), context);
        let closed = Error::transport_error(Error::ConnectionClosed { source: None });
        assert_eq!(closed.kind(), ErrorKind::Transport);
        assert!(!closed.requires_new_session());
    }

    #[test]
    fn test_visca_error_from_code() {
        assert!(matches!(Error::from_code(0x01), Error::MessageLengthError));
        assert!(matches!(Error::from_code(0x02), Error::SyntaxError));
        assert!(matches!(Error::from_code(0x03), Error::CommandBufferFull));
        assert!(matches!(Error::from_code(0x04), Error::CommandCancelled));
        assert!(matches!(Error::from_code(0x05), Error::NoSocket));
        assert!(matches!(
            Error::from_code(0x41),
            Error::CommandNotExecutable
        ));
        assert!(matches!(
            Error::from_code(0xFF),
            Error::Unknown { code: 0xFF }
        ));
    }

    #[test]
    fn test_error_code_mapping_table() {
        // Table-driven test for all known VISCA error codes
        // This ensures consistent mapping across all layers
        let cases = [
            (0x01, "MessageLengthError"),
            (0x02, "SyntaxError"),
            (0x03, "CommandBufferFull"),
            (0x04, "CommandCancelled"),
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
                Error::CommandCancelled => "CommandCancelled",
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
                matches!(error, Error::Unknown { code: c } if c == code),
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
        assert_eq!(Error::CommandCancelled.to_string(), "Command was canceled");
        assert_eq!(Error::NoSocket.to_string(), "No socket available");
        assert_eq!(
            Error::CommandNotExecutable.to_string(),
            "Command is not executable"
        );
        assert_eq!(
            Error::Unknown { code: 0x99 }.to_string(),
            "Unknown error code: 0x99"
        );
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
        assert!(matches!(visca_err, Error::Io { .. }));
    }

    #[test]
    fn test_visca_error_from_nom_error() {
        use nom::error::{Error as NomError, ErrorKind};

        let nom_err = nom::Err::Error(NomError::new(&b"test"[..], ErrorKind::Tag));
        let visca_err = Error::from(nom_err);
        assert!(matches!(visca_err, Error::ParseError { .. }));
    }

    #[test]
    fn test_exhaustive_error_kind_mapping() {
        // Verify specific kind() mappings per issue #501

        // Timeout
        assert_eq!(Error::io_timeout().kind(), ErrorKind::Timeout);
        assert_eq!(Error::MaxRetriesExceeded.kind(), ErrorKind::Timeout);

        // Cancelled
        assert_eq!(Error::CommandCancelled.kind(), ErrorKind::Cancelled);

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
            Error::InvalidState {
                reason: Cow::Borrowed("test")
            }
            .kind(),
            ErrorKind::NotExecutable
        );

        // IoClosed
        assert_eq!(
            Error::ConnectionClosed { source: None }.kind(),
            ErrorKind::IoClosed
        );
        assert_eq!(Error::RuntimeShutdown.kind(), ErrorKind::IoClosed);
        // Transport
        assert_eq!(
            Error::transport_error(io::Error::other("test")).kind(),
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
        assert_eq!(Error::Unknown { code: 0xFF }.kind(), ErrorKind::Protocol);

        // Unsupported
        assert_eq!(Error::MissingRuntime.kind(), ErrorKind::Unsupported);

        // InvalidParameter
        assert_eq!(
            Error::InvalidRequest {
                reason: Cow::Borrowed("test")
            }
            .kind(),
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
            Error::Io {
                source: Arc::new(io::Error::new(io::ErrorKind::TimedOut, "test"))
            }
            .kind(),
            ErrorKind::Timeout
        );
        assert_eq!(
            Error::Io {
                source: Arc::new(io::Error::new(io::ErrorKind::ConnectionRefused, "test"))
            }
            .kind(),
            ErrorKind::IoRefused
        );
        assert_eq!(
            Error::Io {
                source: Arc::new(io::Error::new(io::ErrorKind::ConnectionReset, "test"))
            }
            .kind(),
            ErrorKind::IoClosed
        );
        assert_eq!(
            Error::Io {
                source: Arc::new(io::Error::new(io::ErrorKind::BrokenPipe, "test"))
            }
            .kind(),
            ErrorKind::IoClosed
        );
        assert_eq!(
            Error::Io {
                source: Arc::new(io::Error::other("test"))
            }
            .kind(),
            ErrorKind::Other
        );
    }

    #[test]
    fn requires_new_session_separates_transport_death_from_deliberate_shutdown() {
        // Issue #564: `IoClosed` conflates the three distinct terminal session
        // errors, so the classification must not be derived from the kind.
        let dropped = Error::ConnectionClosed {
            source: Some(Arc::new(Error::from(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "closed by peer",
            )))),
        };
        let poisoned = Error::StreamPoisoned {
            source: Arc::new(Error::from(io::Error::other("partial write"))),
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
            Error::ConnectionClosed { source: None },
            Error::StreamPoisoned {
                source: Arc::new(Error::from(io::Error::other("framing failure"))),
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
            Error::CommandCancelled,
            Error::CommandNotExecutable,
            Error::SyntaxError,
            Error::NoSocket,
            Error::MaxRetriesExceeded,
            Error::CancellationUnconfirmed,
            // Issue #671: a raw command's default unconfirmed outcome fails the
            // one request while the session keeps running.
            Error::UnsequencedCommandUnconfirmed,
            Error::transport_error(io::Error::other("serial encode failed")),
            Error::ConnectionFailed {
                addr: Cow::Borrowed("192.168.1.100:5678"),
                source: Arc::new(io::Error::other("refused")),
            },
            Error::Io {
                source: Arc::new(io::Error::new(io::ErrorKind::BrokenPipe, "pipe")),
            },
        ] {
            assert!(
                !error.requires_new_session(),
                "error should not require a new session: {error}"
            );
        }
    }

    #[test]
    fn requires_new_session_survives_added_context() {
        let error = Error::StreamPoisoned {
            source: Arc::new(Error::from(io::Error::other("partial write"))),
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

        let error5 = Error::Io {
            source: Arc::new(io::Error::other("test io error")),
        };
        let error6 = error5.clone();
        assert!(matches!(error6, Error::Io { .. }));

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
    fn test_with_context_message_format() {
        let error = Error::CommandBufferFull;
        let contextual = error.with_context("Failed to recall preset 5");
        assert_eq!(
            contextual.to_string(),
            "Failed to recall preset 5: Command buffer is full"
        );
    }

    #[test]
    fn test_formatted_with_context_message_format() {
        let preset_id = 5;
        let error = Error::CommandBufferFull;
        let contextual = error.with_context(format!("Failed to recall preset {}", preset_id));
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

#[cfg(test)]
#[path = "error_recovery_tests.rs"]
mod recovery_tests;

#[cfg(all(test, feature = "serde"))]
mod portable_metadata_tests {
    use super::*;

    #[test]
    fn recovery_metadata_preserves_the_delay_and_certainty() -> Result<(), serde_json::Error> {
        let recovery = Error::CommandBufferFull.recovery();
        let json = serde_json::json!({
            "resubmit": {"after": {"secs": 0, "nanos": 200_000_000}}
        });
        assert_eq!(serde_json::to_value(recovery)?, json);
        assert_eq!(serde_json::from_value::<Recovery>(json)?, recovery);

        let context = FailureContext::new(FailureStage::CancellationAttempt, Certainty::StillLive);
        let json = serde_json::json!({
            "stage": "cancellation_attempt", "certainty": "still_live"
        });
        assert_eq!(serde_json::to_value(context)?, json);
        assert_eq!(serde_json::from_value::<FailureContext>(json)?, context);
        assert_eq!(serde_json::to_value(ErrorKind::IoClosed)?, "io_closed");
        assert!(serde_json::from_value::<FailureContext>(serde_json::json!({
            "stage": "terminal", "certainty": "unconfirmed", "retry": true
        }))
        .is_err());
        assert!(serde_json::from_value::<Recovery>(serde_json::json!({
            "resubmit": {"after": {"secs": 1, "nanos": 0}, "retry": true}
        }))
        .is_err());
        Ok(())
    }

    #[test]
    #[cfg(all(feature = "schemars", feature = "ts-rs"))]
    fn recovery_schema_and_typescript_match_the_stored_duration() {
        let schema = schemars::schema_for!(FailureContext);
        assert_eq!(schema.as_value()["additionalProperties"], false);
        assert_eq!(
            schema.as_value()["properties"]["stage"]["$ref"],
            "#/$defs/FailureStage"
        );
        let config = ts_rs::Config::default();
        let shape = |declaration: String| {
            declaration
                .lines()
                .filter(|line| {
                    let line = line.trim_start();
                    !line.starts_with('*') && !line.starts_with("/**")
                })
                .collect::<Vec<_>>()
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        };
        assert_eq!(
            shape(<FailureContext as ts_rs::TS>::decl(&config)),
            "type FailureContext = { stage: FailureStage, certainty: Certainty, };"
        );
        let recovery = shape(<Recovery as ts_rs::TS>::decl(&config));
        assert!(recovery.contains("\"resubmit\": { after: { secs: number; nanos: number }"));
        assert!(recovery.contains("\"reopen_session\""));
    }
}
