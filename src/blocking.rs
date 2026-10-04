//! Blocking owner facade.
//!
//! A [`Session`] owns exactly one native owner worker thread and the transport
//! moved into it (D24). [`Camera`] values are inexpensive target views:
//! cloning one only clones the owner handle and its immutable target/profile
//! facts. Every handle is `Clone + Send + Sync`, and calls from any number of
//! threads are serialized by the worker rather than by the caller. These
//! handles are deliberately mode-native: they expose direct [`Result`]s and
//! never create or return a future. Dropping a non-final handle leaves the
//! shared owner running; dropping the final one stops the worker and releases
//! its transport, but never emits a protocol STOP. [`Session::close`] is the
//! deterministic release barrier.

use std::{
    fmt,
    marker::PhantomData,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use crate::{
    camera::{IdleWait, MotionQuery},
    completion,
    prepared::{
        prepare_command, prepare_inquiry, prepare_operation, prepare_position_queries,
        ClassSelection,
    },
    request::builtin::{FocusStop, PanTiltStop, ZoomStop},
    runtime::owner::{
        ensure_before_deadline, sample_positions_blocking, BlockingOperationReceipt,
        BlockingOwnerHandle, BlockingTransportAdapter,
    },
    stop_request::pan_tilt_stop_request,
    AffectedAxes, CameraId, CancellationOutcome, CompileTimeProfile, DiagnosticSubscription, Error,
    Inquiry, MetricsSnapshot, OperationCommand, OperationalTuning, PlainCommand, ProfileSpec,
    Result, StateCache, SubmissionClass,
};

const MOTION_QUERY_OBSERVER_BUDGET: Duration = Duration::from_secs(30);

pub use crate::OperationId;

/// A blocking handle on one admitted operation.
///
/// The completion marker `K` is the only type parameter. No profile,
/// transport, executor, or runtime type appears in this public handle, and it
/// borrows nothing: it keeps the session's owner alive, so it may outlive the
/// [`Session`] and [`Camera`] it came from and move to another thread. The
/// owner remains authoritative for protocol lifecycle, timeouts, settlement,
/// and cancellation policy; the handle observes it.
///
/// # Waits borrow the handle
///
/// Every wait takes `&mut self`, and the handle caches the authoritative
/// result once received. Waiting again returns the cached result, an
/// [`applied`](Self::applied) wait can be followed by
/// [`settled`](Operation::settled), and a wait that times out releases only
/// that wait: the operation keeps running and the handle keeps observing it.
///
/// # Dropping never stops the camera
///
/// Dropping this handle is exactly [`detach`](Self::detach): it relinquishes
/// observation and nothing else. The owner keeps the protocol state, never
/// interprets handle drop as cancellation, and no STOP is written, so an early
/// `?` return or a panic unwinding past a live handle leaves physical movement
/// running until something ends it. This matches 1.x.
///
/// To bound movement by a scope, write a small guard whose own `Drop` submits
/// the typed STOP — see the guard pattern in `docs/migration_2_0.md` and
/// `examples/operation_handles.rs`. For an explicit stop on a path you
/// control, use `camera.pan_tilt().stop()`, `camera.zoom().stop()`,
/// `camera.focus().stop()`, or `camera.motion().stop_all_motion()`;
/// [`cancel`](Self::cancel) is protocol cancellation and does not by itself
/// prove motion ended.
#[must_use = "observe, cancel, or explicitly detach this operation"]
pub struct Operation<K>
where
    K: completion::Kind,
{
    receipt: BlockingOperationReceipt<K>,
}

impl<K> Operation<K>
where
    K: completion::Kind,
{
    /// Creates a public handle over an admitted owner receipt.
    ///
    /// This constructor is crate-private: only the blocking admission path may
    /// create a lifecycle handle, after the owner admitted the request.
    pub(crate) fn from_receipt(receipt: BlockingOperationReceipt<K>) -> Self {
        Self { receipt }
    }

    /// Returns the opaque identity assigned when this operation was admitted.
    #[must_use]
    pub fn id(&self) -> OperationId {
        OperationId::from_raw(self.receipt.id())
    }

    /// Waits for exact protocol application, bounded by the configured
    /// observer deadline.
    pub fn applied(&mut self) -> Result<(), Error> {
        self.receipt.applied(None)
    }

    /// Waits for exact protocol application, bounded by `timeout`.
    ///
    /// The deadline bounds only this wait. If it expires the wait returns
    /// [`Error::ObservationTimeout`]: the operation keeps running, its
    /// scheduler deadline is unchanged, and the handle can wait again.
    pub fn applied_with_timeout(&mut self, timeout: Duration) -> Result<(), Error> {
        self.receipt.applied(Some(timeout))
    }

    /// Cancels the operation and waits for the cancellation's conclusion,
    /// bounded by the configured cancellation deadline.
    ///
    /// The result is [`CancellationOutcome::Cancelled`] when cancellation
    /// won, [`CancellationOutcome::Completed`] when the operation was applied
    /// first, and the operation's own error when it failed first.
    ///
    /// Cancelling is idempotent. The handle has one cancellation intent;
    /// calling `cancel` again, including after a timed-out call, observes that
    /// intent instead of sending a second cancellation. Once the operation's
    /// outcome is known, `cancel` answers from it without sending anything.
    ///
    /// # A refused cancellation leaves the operation running
    ///
    /// Cancelling a request that is still queued always succeeds. Cancelling
    /// one that has already been written needs profile support for the
    /// standard VISCA socket-cancel command; without it — [`PtzOpticsG2`] is
    /// the only built-in profile in that position — the owner refuses with
    /// [`Error::NotSupported`] and leaves the original request scheduled,
    /// retryable, and able to complete. The handle is unaffected and can
    /// still wait for it. A refusal does not stop the camera; a moving axis
    /// ends with an applied typed STOP.
    ///
    /// A cancellation that was accepted but then failed without ending the
    /// operation (its cancel write failed, or its observation deadline
    /// expired) returns that error; the handle still observes the operation's
    /// own outcome.
    ///
    /// [`PtzOpticsG2`]: crate::profiles::PtzOpticsG2
    pub fn cancel(&mut self) -> Result<CancellationOutcome, Error> {
        self.receipt.cancel(None)
    }

    /// [`cancel`](Self::cancel), bounded by `timeout` instead of the
    /// configured cancellation deadline. If it expires, the cancellation
    /// intent stays recorded and a later `cancel` observes it.
    pub fn cancel_with_timeout(&mut self, timeout: Duration) -> Result<CancellationOutcome, Error> {
        self.receipt.cancel(Some(timeout))
    }

    /// Explicitly relinquishes this operation's observation.
    ///
    /// Detaching never records cancellation and never emits a physical STOP.
    /// It is the explicit spelling of what dropping the handle already does;
    /// movement continues until something else ends it.
    pub fn detach(self) {}
}

impl Operation<completion::Targeted> {
    /// Waits for exact application and the profile-selected protocol
    /// settlement condition, bounded by the configured settlement budget.
    ///
    /// Application is cached, so calling this after
    /// [`applied`](Operation::applied) continues from it. Settlement proven
    /// by position polling is cached once proven; a polling wait that times
    /// out restarts with a fresh proof.
    pub fn settled(&mut self) -> Result<(), Error> {
        self.receipt.settled(None)
    }

    /// [`settled`](Self::settled), bounded by `timeout`.
    ///
    /// The deadline bounds only this wait; the prepared scheduler and
    /// settlement policy are unchanged. Each abandoned polling attempt may
    /// leave its admitted position inquiries running until their own
    /// deadlines, within the session's admission capacity.
    pub fn settled_with_timeout(&mut self, timeout: Duration) -> Result<(), Error> {
        self.receipt.settled(Some(timeout))
    }
}

impl<K> fmt::Debug for Operation<K>
where
    K: completion::Kind,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Operation")
            .field("id", &self.id())
            .finish()
    }
}

/// Re-export the mode-independent profile/target settings from the blocking
/// facade namespace.
pub use crate::SessionConfig;

#[cfg(feature = "blocking")]
mod construction {
    use super::*;

    /// Final owner-backed standard construction namespace.
    ///
    /// Standard network construction is owner-backed and returns a
    /// [`CameraSession`]. Serial and custom transports retain the multi-target
    /// [`Session`] path.
    pub use crate::camera::CameraConfig;

    /// One-line owner-backed blocking connection constructor.
    #[derive(Debug, Clone, Copy)]
    pub struct Connect;

    impl Connect {
        /// Opens a standard TCP or UDP transport selected at runtime.
        ///
        /// Unlike [`Self::open_tcp`] and [`Self::open_udp`], this entry point
        /// only requires [`CompileTimeProfile`]. Transport compatibility and
        /// the profile's default port are resolved before any socket I/O.
        /// Serial transports use `Connect::open_serial`, and custom transports
        /// use [`Session::open`].
        pub fn open<P>(transport: crate::camera::TransportOptions) -> Result<CameraSession<P>>
        where
            P: CompileTimeProfile,
        {
            CameraConfig::<P>::new().transport(transport).open()
        }

        /// Opens one owner-backed blocking TCP camera session.
        pub fn open_tcp<P>(address: impl Into<String>) -> Result<CameraSession<P>>
        where
            P: CompileTimeProfile + crate::capabilities::SupportsTcp,
        {
            CameraConfig::<P>::tcp(address).open()
        }

        /// Opens one owner-backed blocking UDP camera session.
        pub fn open_udp<P>(address: impl Into<String>) -> Result<CameraSession<P>>
        where
            P: CompileTimeProfile + crate::capabilities::SupportsUdp,
        {
            CameraConfig::<P>::udp(address).open()
        }

        /// Opens one owner-backed blocking serial session.
        #[cfg(feature = "transport-serial")]
        pub fn open_serial<P>(port: impl Into<String>, baud_rate: u32) -> Result<Session>
        where
            P: CompileTimeProfile + crate::capabilities::SupportsSerial,
        {
            CameraConfig::<P>::serial(port, baud_rate).open_serial()
        }
    }
}

#[cfg(feature = "blocking")]
pub use construction::{CameraConfig, Connect};

/// One blocking owner session.
///
/// The transport is moved into one native owner worker thread at
/// construction (D24). The session, the [`Camera`] views taken from it, and
/// the [`Operation`] handles they return are all `Clone + Send + Sync` and
/// borrow nothing, so they may be shared freely between threads: the worker
/// serializes every call, and one thread waiting on an operation never blocks
/// another thread's submit, cancel, or STOP. Timers, reads, and correlation
/// cleanup progress on the worker even while no caller is inside the session.
///
/// Every clone shares the one worker. Dropping the last handle — session,
/// camera, or operation — stops it and releases the transport without sending
/// a protocol STOP; [`close`](Self::close) is the explicit, joining teardown.
#[derive(Clone)]
pub struct Session {
    owner: BlockingOwnerHandle,
    config: Arc<SessionConfig>,
}

impl fmt::Debug for Session {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Session")
            .field("targets", &self.config.target_count())
            .finish_non_exhaustive()
    }
}

impl Session {
    /// Opens a blocking session over a configured transport and profile.
    ///
    /// Startup validates the profiles, tuning, transport buffers, and the
    /// immutable owner policy, and sends the optional Sony sequence RESET on
    /// the calling thread, before it starts exactly one worker thread. Any
    /// failure, including a failed thread spawn, drops the transport and
    /// returns the error; no worker is left running.
    pub fn open<T>(transport: T, config: SessionConfig) -> Result<Self, Error>
    where
        T: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + 'static,
    {
        config.validate_for_transport(transport.standard_transport_kind())?;
        let profiles = config.profile_registry();
        let mut adapter = BlockingTransportAdapter::new_with_targets(
            transport,
            &profiles,
            config.tuning(),
            config.admission_capacity(),
            config.strict_unconfirmed_poison(),
        )?;
        if config.sony_sequence_reset_on_connect() {
            adapter.send_sony_sequence_reset()?;
        }
        let policy = adapter.policy().clone();
        Ok(Self {
            owner: BlockingOwnerHandle::spawn(policy, adapter)?,
            config: Arc::new(config),
        })
    }

    /// Returns the statically checked view for the sole registered target.
    pub fn camera<P>(&self) -> Result<Camera<P>, Error>
    where
        P: CompileTimeProfile,
    {
        let target = self.config.sole_target().ok_or_else(|| {
            if self.config.target_count() == 0 {
                Error::InvalidState("session has no registered target".into())
            } else {
                Error::InvalidState(
                    "session has multiple registered targets; select one with camera_for".into(),
                )
            }
        })?;
        self.camera_for::<P>(target)
    }

    /// Returns a target-specific statically checked camera view.
    pub fn camera_for<P>(&self, target: CameraId) -> Result<Camera<P>, Error>
    where
        P: CompileTimeProfile,
    {
        let profile = self.config.profile_arc_for_compile_time::<P>(target)?;
        Ok(Camera::from_core(BlockingCameraCore {
            owner: self.owner.clone(),
            target,
            profile,
            class: ClassSelection::Request,
        }))
    }

    /// Returns the runtime-profile camera view for the sole registered target.
    ///
    /// Unlike [`Self::camera`], this selector does not require a
    /// [`CompileTimeProfile`] marker. Every request is still validated against
    /// the registered [`ProfileSpec`] before encoding or transport I/O.
    ///
    /// # Example: a blocking runtime-only profile
    ///
    /// ```no_run
    /// use std::time::Duration;
    /// use grafton_visca::{
    ///     blocking::Session,
    ///     capabilities::Capabilities,
    ///     command::PowerOn,
    ///     profile::{
    ///         PositionInquirySupport, ProfileEnvelope, ProfileSpec, ProfileTiming,
    ///         TransportCompatibility,
    ///     },
    ///     transport::Transport,
    ///     CommandTimeouts, SessionConfig,
    /// };
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut capabilities = Capabilities::runtime_baseline("Custom camera", 1)?;
    /// capabilities.has_power = true;
    /// capabilities.power_on_time = Duration::from_secs(1);
    ///
    /// let timing = ProfileTiming::builder()
    ///     .ack_timeout(Duration::from_millis(100))
    ///     .command_timeouts(CommandTimeouts::default())
    ///     .inquiry_timeout(Duration::from_secs(1))
    ///     .cancellation_timeout(Duration::from_secs(1))
    ///     .ambiguity_timeout(Duration::from_secs(1))
    ///     .busy_timeout(Duration::ZERO)
    ///     .raw_inquiry_reply_skew(Duration::ZERO)
    ///     .minimum_inquiry_spacing(Duration::ZERO)
    ///     .minimum_command_spacing(Duration::ZERO)
    ///     .build()?;
    /// let profile = ProfileSpec::builder(capabilities)
    ///     .transports(TransportCompatibility::new(Some(5678), None, false))
    ///     .envelope(ProfileEnvelope::RawVisca)
    ///     .timing(timing)
    ///     .maximum_command_sockets(1)
    ///     .supports_operation_complete(true)
    ///     .supports_command_cancel(false)
    ///     .preset_recall_axes(None)
    ///     .position_inquiries(PositionInquirySupport::new(false, false, false))
    ///     .build()?;
    ///
    /// let transport = Transport::tcp()
    ///     .address("192.168.0.110:5678")
    ///     .build_blocking()?;
    /// let session = Session::open(transport, SessionConfig::new(profile))?;
    /// session.camera_dyn()?.execute(&PowerOn::new())?;
    /// session.close()?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "dyn-api")]
    pub fn camera_dyn(&self) -> Result<crate::dynapi::BlockingDynSessionCamera, Error> {
        Ok(crate::dynapi::BlockingDynSessionCamera::new(
            self.camera_core()?,
        ))
    }

    /// Returns a target-specific runtime-profile camera view.
    #[cfg(feature = "dyn-api")]
    pub fn camera_dyn_for(
        &self,
        target: CameraId,
    ) -> Result<crate::dynapi::BlockingDynSessionCamera, Error> {
        Ok(crate::dynapi::BlockingDynSessionCamera::new(
            self.camera_core_for(target)?,
        ))
    }

    #[cfg(feature = "dyn-api")]
    fn camera_core(&self) -> Result<BlockingCameraCore, Error> {
        let target = self.config.sole_target().ok_or_else(|| {
            if self.config.target_count() == 0 {
                Error::InvalidState("session has no registered target".into())
            } else {
                Error::InvalidState(
                    "session has multiple registered targets; select one with camera_dyn_for"
                        .into(),
                )
            }
        })?;
        self.camera_core_for(target)
    }

    #[cfg(feature = "dyn-api")]
    fn camera_core_for(&self, target: CameraId) -> Result<BlockingCameraCore, Error> {
        let profile = self
            .config
            .profile_arc(target)
            .ok_or_else(|| Error::InvalidRequest("session target is not registered".into()))?;
        Ok(BlockingCameraCore {
            owner: self.owner.clone(),
            target,
            profile,
            class: ClassSelection::Request,
        })
    }

    /// Returns the operational tuning this session is currently preparing
    /// requests under.
    ///
    /// This is the live value, not the one the session was opened with: it
    /// reflects the most recent successful [`set_tuning`](Self::set_tuning),
    /// including one made through another clone of this session.
    #[must_use]
    pub fn tuning(&self) -> OperationalTuning {
        self.owner.tuning()
    }

    /// Replaces this session's operational tuning at runtime.
    ///
    /// # Scope
    ///
    /// **Every request prepared after this call returns uses the new values**
    /// — its acknowledgement, completion, settlement, and inquiry deadlines,
    /// its retry budget, and its pacing floor. The owner's session-wide pacing
    /// and per-target command-socket capacity are re-derived immediately, so
    /// work still queued behind pacing is released under the new values too.
    ///
    /// **Requests already in flight keep the deadlines they were admitted
    /// with.** 2.0 stamps a request's deadlines once, at preparation, and the
    /// engine derives its absolute phase deadlines from that stamp; nothing is
    /// re-timed underneath an [`Operation`] a caller is already holding. This
    /// is the one deliberate difference from 1.2.0's
    /// `Camera::set_timeout_config`, which recomputed deadlines on every
    /// housekeeping pass and therefore also covered work in flight. To widen a
    /// deadline for a command that is already running, cancel it and resubmit.
    ///
    /// The update is not a merge: a field left unset returns to its profile
    /// default rather than keeping the value a previous call installed.
    /// Construction-only recovery policy lives on [`SessionConfig`], outside
    /// `OperationalTuning`, and is therefore unaffected by this call.
    ///
    /// The update travels through the owner's control boundary and the owner is
    /// its only writer, so two session clones reconfiguring concurrently
    /// resolve last-writer-wins in the order the owner accepted them; no reader
    /// ever observes a mixture of the two.
    ///
    /// # Errors
    ///
    /// Rejects tuning that construction would reject for a registered profile:
    /// values that weaken pacing minima, raise a socket limit, undercut a
    /// deadline, or specify incoherent retry timing, leaving the session's
    /// current tuning untouched. Returns the session's terminal error if the
    /// owner has shut down; a retained terminal error takes precedence over
    /// validation of the proposed update.
    pub fn set_tuning(&self, tuning: OperationalTuning) -> Result<(), Error> {
        let validated_tuning = self.config.validate_tuning(tuning).map(|()| tuning);
        self.owner.reconfigure(validated_tuning)
    }

    /// Requests owner shutdown.
    ///
    /// This is an idempotent shutdown request. It returns once the request is
    /// accepted by the owner's bounded shutdown boundary; it does not join the
    /// worker or claim that transport teardown has finished. After this call,
    /// new request admission is rejected with [`Error::RuntimeShutdown`]. All
    /// [`Session`] clones share this one shutdown boundary.
    pub fn shutdown(&self) -> Result<(), Error> {
        self.owner.shutdown()
    }

    /// Requests owner shutdown and consumes this session, waiting for teardown.
    ///
    /// Unlike [`Self::shutdown`], this is a deterministic transport teardown
    /// barrier: it returns only after the worker has dropped its transport,
    /// so reopening the same endpoint after `close` is safe. If shutdown was
    /// accepted, the terminal owner result is returned after teardown: an
    /// explicit shutdown returns `Ok(())`, while a transport close or stream
    /// poison that won the race is returned unchanged. If sending this call's
    /// shutdown signal fails immediately, that error is preserved. A worker
    /// that panicked is reported as [`Error::InvalidState`].
    /// A custom transport calling this from its own worker requests shutdown
    /// but receives [`Error::InvalidState`] immediately: it cannot wait for
    /// its own teardown. A caller on another thread can still close and join.
    pub fn close(self) -> Result<(), Error> {
        self.owner.close()
    }

    /// Returns a scalar snapshot of the owner without cloning diagnostics.
    pub fn metrics(&self) -> Result<MetricsSnapshot, Error> {
        self.owner.metrics()
    }

    /// Subscribes to the owner's bounded, best-effort diagnostic stream.
    ///
    /// `capacity` must be in `1..=128`; at most four live subscriptions are
    /// retained by one owner. A slow subscriber never stalls command or
    /// transport processing; its dropped events are visible in [`Self::metrics`].
    pub fn subscribe_diagnostics(&self, capacity: usize) -> Result<DiagnosticSubscription, Error> {
        self.owner
            .subscribe_diagnostics(capacity)
            .map(DiagnosticSubscription::from_owner)
    }
}

/// One blocking owner session that owns exactly one compile-time bound camera.
///
/// This is the single-target counterpart of [`Session`]. The profile is named
/// once, at construction, and the [`Camera`] this type hands out was built from
/// that same profile parameter: the session path's second, runtime-checked
/// profile naming (`session.camera::<P>()?`) has no equivalent here, so a
/// profile mismatch is not expressible.
///
/// The value owns its session, so it is self-sufficient: the connection lives
/// as long as this camera session (or a [`Camera`] taken out of it) is alive.
/// Dropping the final owner handle stops the worker and releases the
/// transport, but does not emit a protocol STOP. Use [`close`](Self::close)
/// for the explicit teardown that mirrors [`Session::close`].
pub struct CameraSession<P: CompileTimeProfile> {
    session: Session,
    camera: Camera<P>,
}

impl<P: CompileTimeProfile> fmt::Debug for CameraSession<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CameraSession")
            .field("target", &self.camera.target())
            .field("profile", &self.camera.profile())
            .finish_non_exhaustive()
    }
}

impl<P: CompileTimeProfile> CameraSession<P> {
    /// Builds the single-camera view for a session whose sole target was
    /// registered from the same `P`.
    ///
    /// This is crate-private and is only reachable from the `P`-typed
    /// construction paths, where the registered [`ProfileSpec`] was lowered
    /// from this exact `P`. That is what makes the bind structural instead of
    /// a runtime profile comparison.
    pub(crate) fn from_session(session: Session, target: CameraId) -> Result<Self, Error> {
        let profile = session.config.profile_arc(target).ok_or_else(|| {
            Error::InvalidState("single-camera session lost its registered target".into())
        })?;
        let camera = Camera::from_core(BlockingCameraCore {
            owner: session.owner.clone(),
            target,
            profile,
            class: ClassSelection::Request,
        });
        Ok(Self { session, camera })
    }

    /// Opens one single-camera owner session over a caller-owned transport.
    ///
    /// This is the [`Session::open`] counterpart for callers that already own
    /// a transport. The profile comes from `config`, so the camera this
    /// session hands out is bound to the same `P` the configuration was
    /// written for.
    pub fn open<T>(transport: T, config: &CameraConfig<P>) -> Result<Self, Error>
    where
        T: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + 'static,
    {
        let target = config.camera_id;
        Self::from_session(Session::open(transport, config.session_config()?)?, target)
    }

    /// Returns this session's compile-time bound camera view.
    ///
    /// The profile is not named again and the call cannot fail.
    pub const fn camera(&self) -> &Camera<P> {
        &self.camera
    }

    /// Returns the submission-class default this session's camera carries.
    ///
    /// See [`Camera::submission_class`].
    #[must_use]
    pub const fn submission_class(&self) -> Option<SubmissionClass> {
        self.camera.submission_class()
    }

    /// Sets the submission-class default for this session's camera.
    ///
    /// This is [`Camera::set_submission_class`] applied to the camera this
    /// session owns, so every view handed out by [`camera`](Self::camera) and
    /// every noun accessor reached through it submits in `class` afterwards.
    /// A [`Camera`] already taken out with [`into_camera`](Self::into_camera)
    /// carries its own copy and is unaffected.
    pub fn set_submission_class(&mut self, class: Option<SubmissionClass>) {
        self.camera.set_submission_class(class);
    }

    /// Consumes this value and returns the owned camera.
    ///
    /// The camera keeps the owner alive, so the connection survives; the
    /// explicit [`close`](Self::close) is given up in exchange.
    pub fn into_camera(self) -> Camera<P> {
        self.camera
    }

    /// Returns the owned session behind this camera.
    #[must_use]
    pub const fn session(&self) -> &Session {
        &self.session
    }

    /// Returns this session's sole camera target.
    #[must_use]
    pub const fn target(&self) -> CameraId {
        self.camera.target()
    }

    /// Returns the operational tuning this session is currently preparing
    /// requests under.
    #[must_use]
    pub fn tuning(&self) -> OperationalTuning {
        self.session.tuning()
    }

    /// Replaces this session's operational tuning at runtime.
    ///
    /// This is [`Session::set_tuning`] on the session this camera owns; see
    /// there for the exact scope, in particular that requests already in flight
    /// keep the deadlines they were admitted with.
    ///
    /// # Errors
    ///
    /// See [`Session::set_tuning`].
    pub fn set_tuning(&self, tuning: OperationalTuning) -> Result<(), Error> {
        self.session.set_tuning(tuning)
    }

    /// Requests owner shutdown without consuming this value.
    ///
    /// This is the idempotent, non-joining signal; use [`Self::close`] when
    /// the worker and owned transport must be fully released before continuing.
    pub fn shutdown(&self) -> Result<(), Error> {
        self.session.shutdown()
    }

    /// Requests owner shutdown and consumes this camera session, waiting for
    /// the worker and its owned transport to be fully released.
    ///
    /// This is the deterministic teardown barrier described by
    /// [`Session::close`]; use [`Self::shutdown`] when only an idempotent,
    /// non-joining signal is required.
    pub fn close(self) -> Result<(), Error> {
        self.session.close()
    }
}

/// Erased owner-backed blocking target view shared by typed and dynamic
/// projections.
///
/// This is crate-private so the public camera type cannot accidentally expose
/// transport details. It is the only place that retains the owner handle and
/// performs request preparation.
#[derive(Clone)]
pub(crate) struct BlockingCameraCore {
    owner: BlockingOwnerHandle,
    target: CameraId,
    profile: Arc<ProfileSpec>,
    class: ClassSelection,
}

impl fmt::Debug for BlockingCameraCore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Camera")
            .field("target", &self.target)
            .field("profile", &self.profile)
            .finish_non_exhaustive()
    }
}

impl BlockingCameraCore {
    #[cfg(feature = "dyn-api")]
    pub(crate) fn into_typed<P>(self) -> Result<Camera<P>, Error>
    where
        P: CompileTimeProfile,
    {
        self.profile.ensure_compile_time::<P>()?;
        Ok(Camera::from_core(self))
    }

    /// Returns this view's fixed camera target.
    pub(crate) const fn target(&self) -> CameraId {
        self.target
    }

    /// Returns this view's validated profile facts.
    pub(crate) fn profile(&self) -> &ProfileSpec {
        self.profile.as_ref()
    }

    /// Returns a cheap live read-only view of this camera's target-local state.
    pub(crate) fn state_cache(&self) -> StateCache {
        self.owner.state_cache(self.target)
    }

    /// Reads the session's live operational tuning.
    ///
    /// This is read once per preparation rather than copied into the view, so a
    /// view taken before [`Session::set_tuning`] still prepares its next
    /// request under the new values (#631).
    fn tuning(&self) -> OperationalTuning {
        self.owner.tuning()
    }

    /// Returns this view's submission-class default, if it carries one.
    pub(crate) const fn submission_class(&self) -> Option<SubmissionClass> {
        self.class.handle_default()
    }

    /// Sets this view's submission-class default.
    pub(crate) fn set_submission_class(&mut self, class: Option<SubmissionClass>) {
        self.class = ClassSelection::from_handle_default(class);
    }

    /// Executes a plain command through the shared owner.
    ///
    /// Ordinary commands wait for their terminal protocol application. A raw
    /// [`crate::raw::RawReplyShape::NoReply`] command instead succeeds once its
    /// local transport write succeeds; it does not claim camera application.
    pub(crate) fn execute<C>(&self, command: &C) -> Result<(), Error>
    where
        C: PlainCommand + ?Sized,
    {
        let prepared = prepare_command(
            command,
            self.target,
            self.profile.as_ref(),
            self.tuning(),
            self.class,
        )?;
        self.owner.submit_command(prepared)?.wait()
    }

    /// Submits a typed inquiry and returns its decoded response.
    pub(crate) fn inquire<Q>(&self, inquiry: &Q) -> Result<Q::Response, Error>
    where
        Q: Inquiry + ?Sized,
    {
        let prepared = prepare_inquiry(
            inquiry,
            self.target,
            self.profile.as_ref(),
            self.tuning(),
            self.class,
        )?;
        self.owner.submit_inquiry(prepared)?.wait()
    }

    /// Admits a typed operation and returns its lifecycle handle once the
    /// owner has accepted it. A transport write failure is reported through
    /// the operation's outcome.
    pub(crate) fn submit<K, O>(&self, operation: &O) -> Result<Operation<K>, Error>
    where
        K: completion::Kind,
        O: OperationCommand<K> + ?Sized,
    {
        let prepared = prepare_operation::<K, _>(
            operation,
            self.target,
            self.profile.as_ref(),
            self.tuning(),
            self.class,
        )?;
        Ok(Operation::from_receipt(
            self.owner.submit_operation(prepared)?,
        ))
    }

    /// Stops every profile-supported pan/tilt, zoom, and focus axis through
    /// this camera's one owner.
    ///
    /// Every supported typed stop is submitted and observed even when an
    /// earlier submission or application fails. The first supported-axis error
    /// is returned only after every supported stop path has had its observation
    /// attempted.
    pub(crate) fn stop_all_motion(&self) -> Result<(), Error> {
        let mut first_error = None;

        if self.profile.supports_axes(AffectedAxes::PAN_TILT) {
            let pan_tilt_result = match self.pan_tilt_stop_request() {
                Ok(stop) => self
                    .submit::<completion::AppliedOnly, _>(&stop)
                    .and_then(|mut operation| operation.applied()),
                Err(error) => Err(error),
            };
            retain_first_error(&mut first_error, pan_tilt_result);
        }

        if self.profile.supports_axes(AffectedAxes::ZOOM) {
            let zoom_result = self
                .submit::<completion::AppliedOnly, _>(&ZoomStop)
                .and_then(|mut operation| operation.applied());
            retain_first_error(&mut first_error, zoom_result);
        }

        if self.profile.supports_axes(AffectedAxes::FOCUS) {
            let focus_result = self
                .submit::<completion::AppliedOnly, _>(&FocusStop)
                .and_then(|mut operation| operation.applied());
            retain_first_error(&mut first_error, focus_result);
        }

        first_error.map_or(Ok(()), Err)
    }

    /// Observes movement on exactly the selected, profile-supported axes.
    ///
    /// Two complete snapshots are taken through ordinary owner inquiries,
    /// separated by at least [`MotionQuery::window`] on the owner clock, and
    /// compared by the shared pure movement detector. The window plus the
    /// observer budget is lowered once to one owner-clock deadline.
    pub(crate) fn is_moving(&self, query: MotionQuery) -> Result<bool, Error> {
        let mut observation = crate::prepared::MotionWindow::new(query)?;
        let queries = prepare_position_queries(
            self.target,
            self.profile.as_ref(),
            self.tuning(),
            query.axes,
        )?;
        let deadline = self
            .owner
            .deadline_after(observation.budget(MOTION_QUERY_OBSERVER_BUDGET)?)?;

        let baseline = sample_positions_blocking(&self.owner, &queries, deadline)?;
        let final_not_before = observation.observe_baseline(baseline, Instant::now(), deadline)?;
        loop {
            let now = Instant::now();
            if now >= final_not_before {
                break;
            }
            thread::sleep(final_not_before.saturating_duration_since(now));
        }
        ensure_before_deadline(deadline)?;
        let started_at = Instant::now();
        let next = sample_positions_blocking(&self.owner, &queries, deadline)?;
        observation.observe_final(next, started_at)
    }

    /// Waits for exactly the selected, profile-supported axes to become idle.
    ///
    /// The timeout is lowered once to one absolute deadline. Every inquiry
    /// admission, reply, and polling interval is bounded by that same
    /// deadline, and the clock is checked before each new inquiry can be
    /// enqueued. The owner worker keeps the session progressing while this
    /// thread sleeps between samples.
    pub(crate) fn wait_until_idle(&self, wait: IdleWait) -> Result<(), Error> {
        let queries =
            prepare_position_queries(self.target, self.profile.as_ref(), self.tuning(), wait.axes)?;
        let deadline = self.owner.deadline_after(wait.timeout)?;
        let mut detector = crate::prepared::MotionDetector::new(wait.axes, wait.tolerance);

        let baseline = sample_positions_blocking(&self.owner, &queries, deadline)?;
        if detector.observe(baseline)? != crate::prepared::MotionState::NeedSample {
            return Err(Error::InvalidState(
                "new movement detector rejected its baseline snapshot".into(),
            ));
        }

        loop {
            ensure_before_deadline(deadline)?;
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(Error::query_timeout());
            }
            thread::sleep(wait.interval.min(remaining));
            ensure_before_deadline(deadline)?;
            let snapshot = sample_positions_blocking(&self.owner, &queries, deadline)?;
            match detector.observe(snapshot)? {
                crate::prepared::MotionState::Settled => return Ok(()),
                crate::prepared::MotionState::Moving => {}
                crate::prepared::MotionState::NeedSample => {
                    return Err(Error::InvalidState(
                        "movement detector lost its baseline snapshot".into(),
                    ));
                }
            }
        }
    }

    fn pan_tilt_stop_request(&self) -> Result<PanTiltStop, Error> {
        pan_tilt_stop_request(self.profile.as_ref())
    }
}

/// A profile-aware camera view onto one [`Session`] target.
///
/// The profile parameter supplies compile-time capability gates; owner and
/// transport state remain in the shared erased core. The view is
/// `Clone + Send + Sync` and borrows nothing: it keeps the session's owner
/// alive, and concurrent calls through it are serialized by the owner worker.
#[must_use]
pub struct Camera<P: CompileTimeProfile> {
    core: BlockingCameraCore,
    _profile: PhantomData<fn() -> P>,
}

impl<P: CompileTimeProfile> Clone for Camera<P> {
    fn clone(&self) -> Self {
        Self {
            core: self.core.clone(),
            _profile: PhantomData,
        }
    }
}

impl<P: CompileTimeProfile> fmt::Debug for Camera<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Camera")
            .field("target", &self.core.target)
            .field("profile", &self.core.profile)
            .finish_non_exhaustive()
    }
}

impl<P: CompileTimeProfile> Camera<P> {
    pub(crate) fn from_core(core: BlockingCameraCore) -> Self {
        Self {
            core,
            _profile: PhantomData,
        }
    }

    pub(crate) const fn core(&self) -> &BlockingCameraCore {
        &self.core
    }

    /// Returns this view's fixed camera target.
    #[must_use]
    pub const fn target(&self) -> CameraId {
        self.core.target()
    }

    /// Returns this view's validated profile facts.
    #[must_use]
    pub fn profile(&self) -> &ProfileSpec {
        self.core.profile()
    }

    /// Returns this view's validated runtime capability inventory.
    ///
    /// This is the runtime discovery view of the same facts the compile-time
    /// marker traits gate: it reports what the profile documents, not
    /// permission to call a typed API.
    #[must_use]
    pub fn capabilities(&self) -> &crate::capabilities::Capabilities {
        self.core.profile().capabilities()
    }

    /// Returns a cheap live read-only view of this camera's target-local state.
    #[must_use]
    pub fn state_cache(&self) -> StateCache {
        self.core.state_cache()
    }

    /// Returns this handle's submission-class default, if it carries one.
    ///
    /// `None` — the initial value — means every request uses its intrinsic
    /// [`crate::ControlClass`]. See
    /// [`set_submission_class`](Self::set_submission_class).
    #[must_use]
    pub const fn submission_class(&self) -> Option<SubmissionClass> {
        self.core.submission_class()
    }

    /// Derives a camera view whose ordinary work uses `class`.
    ///
    /// The original view is unchanged. Commands, inquiries, operations, and
    /// noun methods submitted through the returned view all inherit this
    /// class; intrinsically urgent stops remain urgent.
    pub fn with_submission_class(&self, class: SubmissionClass) -> Self {
        let mut selected = self.clone();
        selected.set_submission_class(Some(class));
        selected
    }

    /// Sets the ordinary-work [`SubmissionClass`] every later submission from
    /// *this handle* uses, or clears it with `None`.
    ///
    /// The owner dispatches ready work from the highest occupied class first
    /// and FIFO within a class, so lowering this default makes the handle's
    /// traffic yield the transport to other handles' ordinary work, and
    /// raising it makes the handle's traffic overtake work that is still
    /// **queued**. It never interrupts, cancels, or reorders a request that
    /// has already been written to the transport, and it does not change the
    /// class of requests submitted before the call.
    ///
    /// The default is a property of *this handle*, not of the camera or the
    /// session: another view onto the same target keeps its own value, and a
    /// [`clone`](Clone::clone) copies the current value and then diverges. It
    /// applies to every request the handle submits — commands,
    /// inquiries, and operations, including the ones the noun accessors submit
    /// — with exactly one exception.
    ///
    /// # Urgent requests are never demoted
    ///
    /// A request the crate classifies [`crate::ControlClass::Urgent`] — the typed
    /// stops [`PanTiltStop`], [`ZoomStop`], [`FocusStop`], and owner-issued
    /// protocol cancellation — ignores this default and stays urgent. A handle demoted to
    /// [`SubmissionClass::Background`] for telemetry polling therefore still
    /// preempts with an emergency stop. Per-submission overrides obey the same
    /// safety floor, and [`SubmissionClass`] deliberately has no urgent
    /// variant for callers to manufacture.
    ///
    /// Owner-internal traffic that no caller submitted — the settlement
    /// polling behind [`Operation::settled`] and the observation inquiries
    /// behind `motion()` — keeps its own built-in class.
    pub fn set_submission_class(&mut self, class: Option<SubmissionClass>) {
        self.core.set_submission_class(class);
    }

    /// Executes a plain command through this camera's shared owner.
    ///
    /// This intentionally has no request-specific `P: Has*` bound. Noun and
    /// accessor methods carry the `Has*` marker bounds that define the
    /// compile-time profile-permission surface; this direct method is the
    /// uniform typed/downstream extension boundary.
    ///
    /// Preparation calls [`crate::Request::validate_for_profile`] before
    /// encoding, owner admission, or transport I/O. Crate-provided requests
    /// submitted directly return [`crate::Error::FeatureNotSupported`] there
    /// when the selected profile lacks their required feature. Raw and other
    /// downstream requests are explicit low-level extensions and may provide
    /// their own profile validation.
    ///
    /// Ordinary commands wait for their terminal protocol application. A raw
    /// [`crate::raw::RawReplyShape::NoReply`] command instead succeeds once its
    /// local transport write succeeds; it does not claim camera application.
    pub fn execute<C>(&self, command: &C) -> Result<(), Error>
    where
        C: PlainCommand + ?Sized,
    {
        self.core.execute(command)
    }

    /// Sends an inquiry and decodes its response through the shared owner.
    ///
    /// This intentionally has no request-specific `P: Has*` bound. Noun and
    /// accessor methods carry the `Has*` marker bounds that define the
    /// compile-time profile-permission surface; this direct method is the
    /// uniform typed/downstream extension boundary.
    ///
    /// Preparation calls [`crate::Request::validate_for_profile`] before
    /// encoding, owner admission, or transport I/O. Crate-provided inquiries
    /// submitted directly return [`crate::Error::FeatureNotSupported`] there
    /// when the selected profile lacks their required feature. Raw and other
    /// downstream requests are explicit low-level extensions and may provide
    /// their own profile validation.
    pub fn inquire<Q>(&self, inquiry: &Q) -> Result<Q::Response, Error>
    where
        Q: Inquiry + ?Sized,
    {
        self.core.inquire(inquiry)
    }

    /// Submits a typed operation and returns its owner-backed handle once the
    /// owner has admitted it.
    pub fn submit<K, O>(&self, operation: &O) -> Result<Operation<K>, Error>
    where
        K: completion::Kind,
        O: OperationCommand<K> + ?Sized,
    {
        self.core.submit(operation)
    }
}

#[path = "blocking_nouns.rs"]
mod blocking_nouns;

pub use blocking_nouns::{
    AdvancedAccessor, ExposureAccessor, FocusAccessor, ImageAccessor, MenuAccessor, MotionAccessor,
    MotionSyncAccessor, NdFilterAccessor, PanTiltAccessor, PowerAccessor, PresetsAccessor,
    SystemAccessor, TallyAccessor, WhiteBalanceAccessor, ZoomAccessor,
};

fn retain_first_error(first_error: &mut Option<Error>, result: Result<(), Error>) {
    if let Err(error) = result {
        if first_error.is_none() {
            *first_error = Some(error);
        }
    }
}

#[cfg(all(test, feature = "test-utils"))]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use crate::{
        capabilities::Capabilities,
        command::CommandKind,
        profile::{PositionInquirySupport, ProfileEnvelope, ProfileTiming, TransportCompatibility},
        testing::testkit::{helpers, ScriptedBlockingTransport, Step},
        transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
        CommandTimeouts,
    };

    struct FailFirstZoomStopSend {
        inner: ScriptedBlockingTransport,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
        failed: bool,
    }

    impl FailFirstZoomStopSend {
        fn new(steps: impl Into<Vec<Step>>) -> (Self, Arc<Mutex<Vec<Vec<u8>>>>) {
            let writes = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    inner: ScriptedBlockingTransport::new(steps),
                    writes: Arc::clone(&writes),
                    failed: false,
                },
                writes,
            )
        }
    }

    impl HasTransportConfig for FailFirstZoomStopSend {
        fn transport_config(&self) -> &TransportConfig {
            self.inner.transport_config()
        }
    }

    impl BlockingTransport for FailFirstZoomStopSend {
        fn send_with_timeout(
            &mut self,
            bytes: &[u8],
            kind: CommandKind,
            timeout: Duration,
        ) -> Result<(), Error> {
            self.writes
                .lock()
                .expect("writes lock")
                .push(bytes.to_vec());
            if !self.failed && bytes.starts_with(&[0x81, 0x01, 0x04, 0x07]) {
                self.failed = true;
                return Err(Error::TransportError(
                    "injected zoom stop send failure".into(),
                ));
            }
            self.inner.send_with_timeout(bytes, kind, timeout)
        }

        fn recv_into_with_timeout(
            &mut self,
            dst: &mut [u8],
            timeout: Duration,
        ) -> Result<usize, Error> {
            self.inner.recv_into_with_timeout(dst, timeout)
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    fn partial_motion_profile(has_zoom: bool, has_focus: bool) -> ProfileSpec {
        let mut capabilities =
            Capabilities::runtime_baseline("Partial motion test camera", 1).expect("baseline");
        capabilities.has_zoom = has_zoom;
        capabilities.has_focus = has_focus;

        ProfileSpec::builder(capabilities)
            .transports(TransportCompatibility::new(Some(5678), None, false))
            .envelope(ProfileEnvelope::RawVisca)
            .timing(
                ProfileTiming::builder()
                    .ack_timeout(Duration::from_millis(100))
                    .command_timeouts(CommandTimeouts::default())
                    .inquiry_timeout(Duration::from_secs(1))
                    .cancellation_timeout(Duration::from_secs(1))
                    .ambiguity_timeout(Duration::from_secs(1))
                    .busy_timeout(Duration::ZERO)
                    .raw_inquiry_reply_skew(Duration::ZERO)
                    .minimum_inquiry_spacing(Duration::ZERO)
                    .minimum_command_spacing(Duration::ZERO)
                    .build()
                    .expect("valid timing"),
            )
            .maximum_command_sockets(1)
            .supports_operation_complete(true)
            .supports_command_cancel(false)
            .preset_recall_axes(None)
            .position_inquiries(PositionInquirySupport::new(false, false, false))
            .build()
            .expect("valid partial motion profile")
    }

    fn partial_motion_core(session: &Session) -> BlockingCameraCore {
        BlockingCameraCore {
            owner: session.owner.clone(),
            target: CameraId::CAMERA_1,
            profile: session
                .config
                .profile_arc(CameraId::CAMERA_1)
                .expect("registered partial motion profile"),
            class: ClassSelection::Request,
        }
    }

    /// Two views taken from the same session drive its one owned transport,
    /// and an operation submitted through either of them is applied rather
    /// than merely being constructed.
    #[test]
    fn camera_views_share_the_session_owner() {
        use crate::testing::testkit::{helpers, ScriptedBlockingTransport};

        let transport = ScriptedBlockingTransport::new([
            helpers::auto_respond_step(),
            helpers::auto_respond_step(),
        ]);
        let probe = transport.clone();
        let config = SessionConfig::from_compile_time::<crate::profiles::PtzOpticsG2>().unwrap();
        let session = Session::open(transport, config).unwrap();

        let first = session.camera::<crate::profiles::PtzOpticsG2>().unwrap();
        let second = session.camera::<crate::profiles::PtzOpticsG2>().unwrap();
        assert_eq!(first.target(), second.target());

        first.zoom().stop().unwrap().applied().unwrap();
        second.zoom().stop().unwrap().applied().unwrap();

        let sent = probe.sent();
        assert_eq!(
            sent,
            vec![
                vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xff],
                vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xff],
            ],
            "both views wrote their zoom stop through the one owned transport"
        );
        session.shutdown().unwrap();
    }

    #[test]
    fn session_open_sends_opt_in_sony_sequence_reset_before_owner_work() {
        let transport = ScriptedBlockingTransport::new([]);
        let probe = transport.clone();
        let config = SessionConfig::from_compile_time::<crate::profiles::SonyFR7>()
            .unwrap()
            .with_sony_sequence_reset_on_connect(true);

        let session = Session::open(transport, config).expect("Sony session opens after RESET");

        assert_eq!(
            probe.sent(),
            vec![vec![0x02, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0x01]]
        );
        session.shutdown().expect("session shutdown");
    }

    #[test]
    fn stop_all_motion_on_a_zoom_only_runtime_profile_writes_only_zoom_stop() {
        let transport = ScriptedBlockingTransport::new([helpers::auto_respond_step()]);
        let probe = transport.clone();
        let session = Session::open(
            transport,
            SessionConfig::new(partial_motion_profile(true, false)),
        )
        .expect("zoom-only session");

        partial_motion_core(&session)
            .stop_all_motion()
            .expect("zoom-only stop succeeds");

        assert_eq!(
            probe.sent(),
            vec![vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xff]],
            "unsupported pan/tilt and focus stops must never reach the wire"
        );
        session.shutdown().expect("session shutdown");
    }

    #[test]
    fn stop_all_motion_on_a_profile_without_motion_axes_is_a_successful_no_op() {
        let transport = ScriptedBlockingTransport::new([]);
        let probe = transport.clone();
        let session = Session::open(
            transport,
            SessionConfig::new(partial_motion_profile(false, false)),
        )
        .expect("no-axis session");

        partial_motion_core(&session)
            .stop_all_motion()
            .expect("no-axis stop is a successful no-op");

        assert!(
            probe.sent().is_empty(),
            "a profile without motion axes must not emit a stop frame"
        );
        session.shutdown().expect("session shutdown");
    }

    #[test]
    fn stop_all_motion_returns_the_first_supported_failure_after_later_stops() {
        let (transport, writes) = FailFirstZoomStopSend::new([helpers::auto_respond_step()]);
        let session = Session::open(
            transport,
            SessionConfig::new(partial_motion_profile(true, true)),
        )
        .expect("zoom-focus session");

        assert!(matches!(
            partial_motion_core(&session).stop_all_motion(),
            Err(Error::TransportError(_))
        ));
        assert_eq!(
            writes.lock().expect("writes lock").clone(),
            vec![
                vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xff],
                vec![0x81, 0x01, 0x04, 0x08, 0x00, 0xff],
            ],
            "the focus stop must follow the failed zoom stop, while unsupported pan/tilt stays absent"
        );
        session.shutdown().expect("session shutdown");
    }
}
