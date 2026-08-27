//! Blocking operation lifecycle handles.
//!
//! These handles are deliberately mode-native: they expose direct
//! [`Result`]s and never create or return a future. The owner remains the
//! protocol and lifecycle authority; a handle only retains one owner receipt
//! and the caller-thread control needed to observe it.

use std::{fmt, marker::PhantomData, sync::Arc, time::Duration};

use crate::{
    camera::{IdleWait, MotionQuery},
    completion,
    prepared::{prepare_position_queries, ClassSelection},
    request::builtin::{FocusStop, PanTiltStop, ZoomStop},
    runtime::owner::{
        sample_positions_blocking, BlockingCancellationReceipt, BlockingControlHost,
        BlockingOperationReceipt, BlockingReceiptControl, BlockingSessionHost,
    },
    stop_request::pan_tilt_stop_request,
    CameraId, CancellationOutcome, CompileTimeProfile, ControlClass, DiagnosticEvent, Error,
    Inquiry, MetricsSnapshot, OperationCommand, OperationalTuning, PlainCommand, ProfileSpec,
    Result, StateCache,
};

const MOTION_QUERY_OBSERVER_BUDGET: Duration = Duration::from_secs(30);

pub use crate::OperationId;

/// A linear blocking operation handle.
///
/// The lifetime ties the handle to the caller-thread session/owner control;
/// the completion marker is the only type parameter. No profile, transport,
/// executor, or runtime type appears in this public handle.
///
/// # Dropping never stops the camera
///
/// Dropping this handle is exactly [`detach`](Self::detach): it relinquishes
/// the observer and nothing else. The owner keeps the protocol state, never
/// interprets handle drop as cancellation, and no STOP is written, so an early
/// `?` return or a panic unwinding past a live handle leaves physical movement
/// running until something ends it. This matches 1.x exactly and is not a 2.0
/// behaviour change.
///
/// To bound movement by a scope, write a small guard whose own `Drop` submits
/// the typed STOP — see the guard pattern in `docs/migration_2_0.md` and
/// `examples/operation_handles.rs`. For an explicit stop on a path you
/// control, use `camera.pan_tilt().stop()`, `camera.zoom().stop()`,
/// `camera.focus().stop()`, or `camera.motion().stop_all_motion()`;
/// [`cancel`](Self::cancel) records protocol cancellation but does not by
/// itself prove motion ended.
#[must_use = "observe, cancel, or explicitly detach this operation"]
pub struct Operation<'session, K>
where
    K: completion::Kind,
{
    receipt: Option<BlockingOperationReceipt<K>>,
    /// Shared caller-thread owner control.  The handle never holds an
    /// exclusive borrow for its lifetime; each consuming terminal method
    /// creates a short-lived receipt control from this reference.
    host: &'session dyn BlockingControlHost,
    id: OperationId,
    marker: PhantomData<fn() -> K>,
}

impl<'session, K> Operation<'session, K>
where
    K: completion::Kind,
{
    /// Creates a public handle over an admitted owner receipt and its
    /// caller-thread observation control.
    ///
    /// This constructor is crate-private: only the blocking admission path may
    /// create a lifecycle handle, after the owner admitted the request. The
    /// request is either already written or queued behind the target's busy
    /// sockets; a queued request is written by a later owner turn.
    pub(crate) fn from_receipt(
        receipt: BlockingOperationReceipt<K>,
        host: &'session dyn BlockingControlHost,
    ) -> Self {
        let id = OperationId::from_raw(receipt.id());
        Self {
            receipt: Some(receipt),
            host,
            id,
            marker: PhantomData,
        }
    }

    /// Returns the opaque identity assigned when this operation was admitted.
    #[must_use]
    pub fn id(&self) -> OperationId {
        self.id
    }

    /// Waits for exact protocol application using the configured observer
    /// deadline.
    pub fn applied(self) -> Result<(), Error> {
        let (receipt, mut control) = self.take_parts()?;
        receipt.applied(&mut control)
    }

    /// Waits for exact protocol application using an explicit observer
    /// deadline.
    pub fn applied_with_timeout(self, timeout: Duration) -> Result<(), Error> {
        let (receipt, mut control) = self.take_parts()?;
        receipt.applied_with_timeout(&mut control, timeout)
    }

    /// Records cancellation intent and returns its exact terminal observer.
    pub fn cancel(self) -> Result<Cancellation<'session>, Error> {
        let host = self.host;
        let (receipt, mut control) = self.take_parts()?;
        let cancellation = control.cancel_operation(receipt)?;
        Ok(Cancellation::from_receipt(cancellation, host))
    }

    /// Explicitly relinquishes this operation's observation right.
    ///
    /// Detaching never records cancellation and never emits a physical STOP.
    /// It is the explicit spelling of what dropping the handle already does;
    /// movement continues until something else ends it.
    pub fn detach(self) {}

    fn take_parts(
        mut self,
    ) -> Result<
        (
            BlockingOperationReceipt<K>,
            BlockingReceiptControl<'session>,
        ),
        Error,
    > {
        let receipt = self
            .receipt
            .take()
            .ok_or_else(|| Error::InvalidState("operation handle was already consumed".into()))?;
        Ok((receipt, BlockingReceiptControl::shared(self.host)))
    }
}

impl Operation<'_, completion::Targeted> {
    /// Waits for exact application and physical settling using the configured
    /// settlement plan and observer deadline.
    pub fn settled(self) -> Result<(), Error> {
        let (receipt, control) = self.take_parts()?;
        receipt.settled(control).wait().map(drop)
    }

    /// Waits for exact application and physical settling using an explicit
    /// observer deadline.
    pub fn settled_with_timeout(self, timeout: Duration) -> Result<(), Error> {
        let (receipt, control) = self.take_parts()?;
        receipt
            .settled_with_timeout(control, timeout)
            .wait()
            .map(drop)
    }
}

impl<K> Drop for Operation<'_, K>
where
    K: completion::Kind,
{
    fn drop(&mut self) {
        // Dropping the receipt relinquishes observation only. The owner keeps
        // protocol state and never interprets handle drop as cancellation.
        let _ = self.receipt.take();
    }
}

impl<K> fmt::Debug for Operation<'_, K>
where
    K: completion::Kind,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Operation")
            .field("id", &self.id)
            .finish()
    }
}

/// A linear blocking cancellation observer for one exact operation.
#[must_use = "observe or explicitly detach this cancellation"]
pub struct Cancellation<'session> {
    receipt: Option<BlockingCancellationReceipt>,
    host: &'session dyn BlockingControlHost,
}

impl<'session> Cancellation<'session> {
    /// Creates a cancellation observer over the same caller-thread owner as
    /// the operation that produced it.
    pub(crate) fn from_receipt(
        receipt: BlockingCancellationReceipt,
        host: &'session dyn BlockingControlHost,
    ) -> Self {
        Self {
            receipt: Some(receipt),
            host,
        }
    }

    /// Observes whether cancellation won or the original operation completed
    /// first, bounded by the supplied observer deadline.
    pub fn outcome(self, timeout: Duration) -> Result<CancellationOutcome, Error> {
        let (receipt, mut control) = self.take_parts()?;
        receipt.outcome(&mut control, timeout)
    }

    /// Explicitly relinquishes cancellation observation.
    pub fn detach(self) {}

    fn take_parts(
        mut self,
    ) -> Result<
        (
            BlockingCancellationReceipt,
            BlockingReceiptControl<'session>,
        ),
        Error,
    > {
        let receipt = self.receipt.take().ok_or_else(|| {
            Error::InvalidState("cancellation handle was already consumed".into())
        })?;
        Ok((receipt, BlockingReceiptControl::shared(self.host)))
    }
}

impl fmt::Debug for Cancellation<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Cancellation").finish()
    }
}

impl Drop for Cancellation<'_> {
    fn drop(&mut self) {
        // Cancellation intent, once recorded, is owner state. Dropping this
        // observer never sends another cancellation or a physical STOP.
        let _ = self.receipt.take();
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
    /// Blocking construction is owner-backed and returns a [`Session`].
    pub use crate::camera::CameraConfig;

    /// One-line owner-backed blocking connection constructor.
    #[derive(Debug, Clone, Copy)]
    pub struct Connect;

    /// Runtime-free typed blocking connection builder.
    #[derive(Debug, Clone, Copy)]
    pub struct ConnectBuilder;

    /// TCP-selected blocking connection builder.
    #[derive(Debug, Clone)]
    pub struct TcpConnectBuilder {
        address: String,
        use_default_port: bool,
    }

    /// UDP-selected blocking connection builder.
    #[derive(Debug, Clone)]
    pub struct UdpConnectBuilder {
        address: String,
        use_default_port: bool,
    }

    #[cfg(feature = "transport-serial")]
    /// Serial-selected blocking connection builder.
    #[derive(Debug, Clone)]
    pub struct SerialConnectBuilder {
        port: String,
        baud_rate: u32,
    }

    impl Connect {
        /// Opens one owner-backed blocking TCP session.
        pub fn open_tcp<P>(address: impl Into<String>) -> Result<Session>
        where
            P: CompileTimeProfile + crate::capabilities::SupportsTcp,
        {
            CameraConfig::<P>::tcp(address).open()
        }

        /// Opens one owner-backed blocking UDP session.
        pub fn open_udp<P>(address: impl Into<String>) -> Result<Session>
        where
            P: CompileTimeProfile + crate::capabilities::SupportsUdp,
        {
            CameraConfig::<P>::udp(address).open()
        }

        /// Opens one owner-backed blocking TCP session for a single camera.
        ///
        /// The profile is named once and bound at compile time: the returned
        /// [`CameraSession`] hands out the `P` camera
        /// view directly, with no second, runtime-checked profile naming.
        pub fn open_tcp_camera<P>(address: impl Into<String>) -> Result<CameraSession<P>>
        where
            P: CompileTimeProfile + crate::capabilities::SupportsTcp,
        {
            CameraConfig::<P>::tcp(address).open_camera()
        }

        /// Opens one owner-backed blocking UDP session for a single camera.
        ///
        /// The profile is named once and bound at compile time: the returned
        /// [`CameraSession`] hands out the `P` camera
        /// view directly, with no second, runtime-checked profile naming.
        pub fn open_udp_camera<P>(address: impl Into<String>) -> Result<CameraSession<P>>
        where
            P: CompileTimeProfile + crate::capabilities::SupportsUdp,
        {
            CameraConfig::<P>::udp(address).open_camera()
        }

        /// Opens one owner-backed blocking serial session.
        #[cfg(feature = "transport-serial")]
        pub fn open_serial<P>(port: impl Into<String>, baud_rate: u32) -> Result<Session>
        where
            P: CompileTimeProfile + crate::capabilities::SupportsSerial,
        {
            CameraConfig::<P>::serial(port, baud_rate).open_serial()
        }

        /// Creates a typed blocking connection builder.
        pub fn builder() -> ConnectBuilder {
            ConnectBuilder
        }
    }

    impl ConnectBuilder {
        /// Select TCP.
        pub fn tcp(self, address: impl Into<String>) -> TcpConnectBuilder {
            TcpConnectBuilder {
                address: address.into(),
                use_default_port: false,
            }
        }

        /// Select UDP.
        pub fn udp(self, address: impl Into<String>) -> UdpConnectBuilder {
            UdpConnectBuilder {
                address: address.into(),
                use_default_port: false,
            }
        }

        /// Select blocking serial.
        #[cfg(feature = "transport-serial")]
        pub fn serial(self, port: impl Into<String>, baud_rate: u32) -> SerialConnectBuilder {
            SerialConnectBuilder {
                port: port.into(),
                baud_rate,
            }
        }
    }

    impl TcpConnectBuilder {
        /// Apply the selected profile's default TCP port.
        pub fn with_default_port(mut self) -> Self {
            self.use_default_port = true;
            self
        }

        /// Opens through the owner-backed blocking session.
        pub fn open<P>(self) -> Result<Session>
        where
            P: CompileTimeProfile + crate::capabilities::SupportsTcp,
        {
            let config = if self.use_default_port {
                CameraConfig::<P>::tcp(self.address)
            } else {
                CameraConfig::<P>::new()
                    .transport(crate::camera::TransportOptions::tcp(self.address))
                    .without_network_default_port()
            };
            config.open()
        }
    }

    impl UdpConnectBuilder {
        /// Apply the selected profile's default UDP port.
        pub fn with_default_port(mut self) -> Self {
            self.use_default_port = true;
            self
        }

        /// Opens through the owner-backed blocking session.
        pub fn open<P>(self) -> Result<Session>
        where
            P: CompileTimeProfile + crate::capabilities::SupportsUdp,
        {
            let config = if self.use_default_port {
                CameraConfig::<P>::udp(self.address)
            } else {
                CameraConfig::<P>::new()
                    .transport(crate::camera::TransportOptions::udp(self.address))
                    .without_network_default_port()
            };
            config.open()
        }
    }

    #[cfg(feature = "transport-serial")]
    impl SerialConnectBuilder {
        /// Opens through the owner-backed blocking session.
        pub fn open<P>(self) -> Result<Session>
        where
            P: CompileTimeProfile + crate::capabilities::SupportsSerial,
        {
            CameraConfig::<P>::serial(self.port, self.baud_rate).open_serial()
        }
    }
}

#[cfg(all(feature = "blocking", feature = "transport-serial"))]
pub use construction::SerialConnectBuilder;
#[cfg(feature = "blocking")]
pub use construction::{
    CameraConfig, Connect, ConnectBuilder, TcpConnectBuilder, UdpConnectBuilder,
};

/// One caller-driven blocking owner session.
///
/// The transport is moved into one serialized owner at construction. Camera
/// views borrow that owner and may coexist on the caller thread; retaining
/// multiple operation handles never creates a second protocol authority.
#[derive(Debug)]
pub struct Session {
    host: BlockingSessionHost,
    config: SessionConfig,
}

impl Session {
    /// Opens a blocking session over a configured transport and profile.
    pub fn open<T>(transport: T, config: SessionConfig) -> Result<Self, Error>
    where
        T: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + 'static,
    {
        config.validate_for_transport(transport.standard_transport_kind())?;
        let profiles = config.profile_registry();
        let adapter = crate::runtime::owner::BlockingTransportAdapter::new_with_profile_registry(
            transport,
            &profiles,
            config.tuning(),
        )?;
        Ok(Self {
            host: BlockingSessionHost::from_adapter(adapter)?,
            config,
        })
    }

    /// Returns the statically checked view for the sole registered target.
    pub fn camera<P>(&self) -> Result<Camera<'_, P>, Error>
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
    pub fn camera_for<P>(&self, target: CameraId) -> Result<Camera<'_, P>, Error>
    where
        P: CompileTimeProfile,
    {
        let profile = self.config.profile_for_compile_time::<P>(target)?;
        Ok(Camera::from_core(BlockingCameraCore {
            host: &self.host,
            target,
            profile,
            class: ClassSelection::Request,
        }))
    }

    /// Returns the operational tuning this session is currently preparing
    /// requests under.
    ///
    /// This is the live value, not the one the session was opened with: it
    /// reflects the most recent successful [`set_tuning`](Self::set_tuning).
    #[must_use]
    pub fn tuning(&self) -> OperationalTuning {
        self.host.tuning()
    }

    /// Replaces this session's operational tuning at runtime.
    ///
    /// # Scope
    ///
    /// **Every request prepared after this call uses the new values** — its
    /// acknowledgement, completion, settlement, and inquiry deadlines, its
    /// retry budget, and its pacing floor. The owner's session-wide pacing and
    /// per-target command-socket capacity are re-derived immediately, so work
    /// still queued behind pacing is released under the new values too.
    ///
    /// **Requests already in flight keep the deadlines they were admitted
    /// with.** 2.0 stamps a request's deadlines once, at preparation, and the
    /// engine derives its absolute phase deadlines from that stamp; nothing is
    /// re-timed underneath a receipt a caller is already holding. This is the
    /// one deliberate difference from 1.2.0's `Camera::set_timeout_config`,
    /// which recomputed deadlines on every housekeeping pass and therefore also
    /// covered work in flight. To widen a deadline for a command that is
    /// already running, cancel it and resubmit.
    ///
    /// The update is not a merge: `tuning` replaces the previous value whole,
    /// so a field left unset returns to its profile default rather than keeping
    /// the value a previous call installed.
    ///
    /// # Errors
    ///
    /// Rejects exactly what construction rejects — tuning that weakens a
    /// registered profile's pacing minima, raises its socket limit, undercuts
    /// its deadlines, or specifies incoherent retry timing — leaving the
    /// session's current tuning untouched. Also returns
    /// [`Error::TransportBusy`] if called re-entrantly from inside another
    /// owner turn, and the session's terminal error if the owner is gone.
    pub fn set_tuning(&self, tuning: OperationalTuning) -> Result<(), Error> {
        self.config.validate_tuning(tuning)?;
        self.host.reconfigure(tuning)
    }

    /// Requests owner shutdown.
    pub fn shutdown(&self) -> Result<(), Error> {
        self.host.shutdown()
    }

    /// Explicitly shuts down this session.
    pub fn close(self) -> Result<(), Error> {
        self.host.shutdown()
    }

    /// Returns a scalar snapshot of the owner without cloning diagnostics.
    pub fn metrics(&self) -> Result<MetricsSnapshot, Error> {
        self.host.metrics()
    }

    /// Drains the bounded diagnostic ring in owner order.
    ///
    /// The returned vector is at most the owner's fixed diagnostic-ring bound
    /// (128 by default). Calling this repeatedly after an empty drain is safe
    /// and returns an empty vector until another event is recorded.
    pub fn drain_diagnostics(&self) -> Result<Vec<DiagnosticEvent>, Error> {
        self.host.drain_diagnostics().map(|events| {
            events
                .into_iter()
                .map(DiagnosticEvent::from_owner)
                .collect()
        })
    }
}

/// One blocking owner session that owns exactly one compile-time bound camera.
///
/// This is the single-target counterpart of [`Session`]. The profile is named
/// once, at construction, and [`camera`](Self::camera) returns the view for
/// that same profile parameter: the session path's second, runtime-checked
/// profile naming (`session.camera::<P>()?`) has no equivalent here, so a
/// profile mismatch is not expressible.
///
/// The value owns its session, so it is self-sufficient: dropping it drops the
/// owner and its transport. Use [`close`](Self::close) for the explicit
/// teardown that mirrors [`Session::close`].
///
/// The blocking [`Camera`] borrows its session, so the view is handed out by
/// [`camera`](Self::camera) rather than stored; the async facade's owned
/// `Camera<P>` needs no such step.
pub struct CameraSession<P: CompileTimeProfile> {
    session: Session,
    target: CameraId,
    profile: Arc<ProfileSpec>,
    class: ClassSelection,
    _profile: PhantomData<fn() -> P>,
}

impl<P: CompileTimeProfile> fmt::Debug for CameraSession<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CameraSession")
            .field("target", &self.target)
            .field("profile", &self.profile)
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
        Ok(Self {
            session,
            target,
            profile,
            class: ClassSelection::Request,
            _profile: PhantomData,
        })
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
    pub fn camera(&self) -> Camera<'_, P> {
        Camera::from_core(BlockingCameraCore {
            host: &self.session.host,
            target: self.target,
            profile: self.profile.as_ref(),
            class: self.class,
        })
    }

    /// Returns the submission-class default this session hands to its camera.
    ///
    /// See [`Camera::command_class`].
    #[must_use]
    pub const fn command_class(&self) -> Option<ControlClass> {
        self.class.handle_default()
    }

    /// Sets the submission-class default this session hands to its camera.
    ///
    /// The blocking [`Camera`] is built afresh by [`camera`](Self::camera)
    /// rather than stored, so this is how a single-camera session carries a
    /// default: every view taken after this call, and every noun accessor
    /// reached through one, submits in `class`. See
    /// [`Camera::set_command_class`] for the full semantics, including the
    /// rule that an urgent request is never demoted.
    pub fn set_command_class(&mut self, class: Option<ControlClass>) {
        self.class = ClassSelection::from_handle_default(class);
    }

    /// Returns the owned session behind this camera.
    #[must_use]
    pub const fn session(&self) -> &Session {
        &self.session
    }

    /// Returns this session's sole camera target.
    #[must_use]
    pub const fn target(&self) -> CameraId {
        self.target
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
    pub fn shutdown(&self) -> Result<(), Error> {
        self.session.shutdown()
    }

    /// Explicitly shuts down this camera session.
    pub fn close(self) -> Result<(), Error> {
        self.session.close()
    }
}

/// Erased owner-backed blocking target view shared by typed projections.
#[derive(Clone, Copy)]
pub(crate) struct BlockingCameraCore<'session> {
    host: &'session BlockingSessionHost,
    target: CameraId,
    profile: &'session ProfileSpec,
    class: ClassSelection,
}

impl fmt::Debug for BlockingCameraCore<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Camera")
            .field("target", &self.target)
            .field("profile", &self.profile)
            .finish_non_exhaustive()
    }
}

impl<'session> BlockingCameraCore<'session> {
    /// Returns this view's fixed camera target.
    #[must_use]
    pub const fn target(&self) -> CameraId {
        self.target
    }

    /// Returns this view's validated profile facts.
    #[must_use]
    pub const fn profile(&self) -> &ProfileSpec {
        self.profile
    }

    /// Returns a cheap live read-only view of this camera's target-local state.
    #[must_use]
    pub fn state_cache(&self) -> StateCache {
        self.host.state_cache(self.target)
    }

    /// Reads the session's live operational tuning.
    ///
    /// This is read once per preparation rather than copied into the view, so a
    /// view taken before [`Session::set_tuning`] still prepares its next
    /// request under the new values (#631).
    fn tuning(&self) -> OperationalTuning {
        self.host.tuning()
    }

    /// Returns this view's submission-class default, if it carries one.
    pub const fn command_class(&self) -> Option<ControlClass> {
        self.class.handle_default()
    }

    /// Sets this view's submission-class default.
    pub fn set_command_class(&mut self, class: Option<ControlClass>) {
        self.class = ClassSelection::from_handle_default(class);
    }

    /// Executes a plain command and waits for exact application.
    pub fn execute<C>(&self, command: &C) -> Result<(), Error>
    where
        C: PlainCommand + ?Sized,
    {
        self.execute_with_selection(command, self.class)
    }

    /// Executes a plain command in an explicitly named scheduling lane.
    pub fn execute_with_class<C>(&self, command: &C, class: ControlClass) -> Result<(), Error>
    where
        C: PlainCommand + ?Sized,
    {
        self.execute_with_selection(command, ClassSelection::Explicit(class))
    }

    fn execute_with_selection<C>(&self, command: &C, class: ClassSelection) -> Result<(), Error>
    where
        C: PlainCommand + ?Sized,
    {
        let prepared = crate::prepared::prepare_command(
            command,
            self.target,
            self.profile,
            self.tuning(),
            class,
        )?;
        let receipt = self.host.submit_command(prepared)?;
        let mut control = BlockingReceiptControl::shared(self.host);
        receipt.wait(&mut control)
    }

    /// Submits a typed inquiry and returns its decoded response.
    pub fn inquire<Q>(&self, inquiry: &Q) -> Result<Q::Response, Error>
    where
        Q: Inquiry + ?Sized,
    {
        self.inquire_with_selection(inquiry, self.class)
    }

    /// Submits a typed inquiry in an explicitly named scheduling lane.
    pub fn inquire_with_class<Q>(
        &self,
        inquiry: &Q,
        class: ControlClass,
    ) -> Result<Q::Response, Error>
    where
        Q: Inquiry + ?Sized,
    {
        self.inquire_with_selection(inquiry, ClassSelection::Explicit(class))
    }

    fn inquire_with_selection<Q>(
        &self,
        inquiry: &Q,
        class: ClassSelection,
    ) -> Result<Q::Response, Error>
    where
        Q: Inquiry + ?Sized,
    {
        let prepared = crate::prepared::prepare_inquiry(
            inquiry,
            self.target,
            self.profile,
            self.tuning(),
            class,
        )?;
        let receipt = self.host.submit_inquiry(prepared)?;
        let mut control = BlockingReceiptControl::shared(self.host);
        receipt.wait(&mut control)
    }

    /// Admits a typed operation and returns its linear lifecycle handle. The
    /// initial transport write happens here whenever the request wins the
    /// dispatch race; otherwise the request stays queued and is written by a
    /// later owner turn.
    pub fn submit<K, O>(&self, operation: &O) -> Result<Operation<'session, K>, Error>
    where
        K: completion::Kind,
        O: OperationCommand<K> + ?Sized,
    {
        self.submit_with_selection::<K, O>(operation, self.class)
    }

    /// Admits a typed operation in an explicitly named scheduling lane.
    pub fn submit_with_class<K, O>(
        &self,
        operation: &O,
        class: ControlClass,
    ) -> Result<Operation<'session, K>, Error>
    where
        K: completion::Kind,
        O: OperationCommand<K> + ?Sized,
    {
        self.submit_with_selection::<K, O>(operation, ClassSelection::Explicit(class))
    }

    fn submit_with_selection<K, O>(
        &self,
        operation: &O,
        class: ClassSelection,
    ) -> Result<Operation<'session, K>, Error>
    where
        K: completion::Kind,
        O: OperationCommand<K> + ?Sized,
    {
        let prepared = crate::prepared::prepare_operation::<K, _>(
            operation,
            self.target,
            self.profile,
            self.tuning(),
            class,
        )?;
        let receipt = self.host.submit_operation(prepared)?;
        Ok(Operation::from_receipt(receipt, self.host))
    }

    /// Stops pan/tilt, zoom, and focus through this camera's one owner.
    ///
    /// Every typed stop is submitted and observed even when an earlier
    /// submission or application fails. The first error is returned only after
    /// all three stop paths have had their observation attempted.
    pub fn stop_all_motion(&self) -> Result<(), Error> {
        let mut first_error = None;

        let pan_tilt_result = match self.pan_tilt_stop_request() {
            Ok(stop) => self
                .submit::<completion::AppliedOnly, _>(&stop)
                .and_then(|operation| operation.applied()),
            Err(error) => Err(error),
        };
        retain_first_error(&mut first_error, pan_tilt_result);

        let zoom_result = self
            .submit::<completion::AppliedOnly, _>(&ZoomStop)
            .and_then(|operation| operation.applied());
        retain_first_error(&mut first_error, zoom_result);

        let focus_result = self
            .submit::<completion::AppliedOnly, _>(&FocusStop)
            .and_then(|operation| operation.applied());
        retain_first_error(&mut first_error, focus_result);

        first_error.map_or(Ok(()), Err)
    }

    /// Observes movement on exactly the selected, profile-supported axes.
    ///
    /// Two complete snapshots are taken through ordinary owner inquiries and
    /// compared by the shared pure movement detector. The observer budget is
    /// lowered once to one owner-clock deadline.
    pub fn is_moving(&self, query: MotionQuery) -> Result<bool, Error> {
        let queries =
            prepare_position_queries(self.target, self.profile, self.tuning(), query.axes)?;
        let deadline = self.host.deadline_after(MOTION_QUERY_OBSERVER_BUDGET)?;
        let mut control = BlockingReceiptControl::shared(self.host);
        let mut detector = crate::prepared::MotionDetector::new(query.axes, query.tolerance);

        let baseline = sample_positions_blocking(&mut control, &queries, deadline)?;
        if detector.observe(baseline)? != crate::prepared::MotionState::NeedSample {
            return Err(Error::InvalidState(
                "new movement detector rejected its baseline snapshot".into(),
            ));
        }
        ensure_before_deadline(&control, deadline)?;
        let next = sample_positions_blocking(&mut control, &queries, deadline)?;
        Ok(matches!(
            detector.observe(next)?,
            crate::prepared::MotionState::Moving
        ))
    }

    /// Waits for exactly the selected, profile-supported axes to become idle.
    ///
    /// The timeout is lowered once to one absolute owner-clock deadline. Every
    /// inquiry admission, reply, and polling interval is bounded by that same
    /// deadline, and the clock is checked before each new inquiry can be
    /// enqueued.
    pub fn wait_until_idle(&self, wait: IdleWait) -> Result<(), Error> {
        let queries =
            prepare_position_queries(self.target, self.profile, self.tuning(), wait.axes)?;
        let deadline = self.host.deadline_after(wait.timeout)?;
        let mut control = BlockingReceiptControl::shared(self.host);
        let mut detector = crate::prepared::MotionDetector::new(wait.axes, wait.tolerance);

        let baseline = sample_positions_blocking(&mut control, &queries, deadline)?;
        if detector.observe(baseline)? != crate::prepared::MotionState::NeedSample {
            return Err(Error::InvalidState(
                "new movement detector rejected its baseline snapshot".into(),
            ));
        }

        loop {
            ensure_before_deadline(&control, deadline)?;
            let remaining = deadline.saturating_duration_since(control.now());
            if remaining.is_zero() {
                return Err(Error::Timeout);
            }
            control.sleep(wait.interval.min(remaining));
            ensure_before_deadline(&control, deadline)?;
            let snapshot = sample_positions_blocking(&mut control, &queries, deadline)?;
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
        pan_tilt_stop_request(self.profile)
    }
}

/// A profile-aware camera view borrowing one [`Session`] owner.
///
/// The profile parameter supplies compile-time capability gates; owner and
/// transport state remain in the shared borrowed core.
#[must_use]
pub struct Camera<'session, P: CompileTimeProfile> {
    core: BlockingCameraCore<'session>,
    _profile: PhantomData<fn() -> P>,
}

impl<P: CompileTimeProfile> fmt::Debug for Camera<'_, P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Camera")
            .field("target", &self.core.target)
            .field("profile", &self.core.profile)
            .finish_non_exhaustive()
    }
}

impl<'session, P: CompileTimeProfile> Camera<'session, P> {
    pub(crate) fn from_core(core: BlockingCameraCore<'session>) -> Self {
        Self {
            core,
            _profile: PhantomData,
        }
    }

    pub(crate) const fn core(&self) -> &BlockingCameraCore<'session> {
        &self.core
    }

    /// Returns this view's fixed camera target.
    #[must_use]
    pub const fn target(&self) -> CameraId {
        self.core.target()
    }

    /// Returns this view's validated profile facts.
    #[must_use]
    pub const fn profile(&self) -> &ProfileSpec {
        self.core.profile()
    }

    /// Returns this view's validated runtime capability inventory.
    ///
    /// This is the runtime discovery view of the same facts the compile-time
    /// marker traits gate: it reports what the profile documents, not
    /// permission to call a typed API.
    #[must_use]
    pub const fn capabilities(&self) -> &crate::capabilities::Capabilities {
        self.core.profile().capabilities()
    }

    /// Returns a cheap live read-only view of this camera's target-local state.
    #[must_use]
    pub fn state_cache(&self) -> StateCache {
        self.core.state_cache()
    }

    /// Returns this handle's submission-class default, if it carries one.
    ///
    /// `None` — the initial value — means every request is submitted in the
    /// lane its own [`ControlClass`] names. See
    /// [`set_command_class`](Self::set_command_class).
    #[must_use]
    pub const fn command_class(&self) -> Option<ControlClass> {
        self.core.command_class()
    }

    /// Sets the [`ControlClass`] every later submission from *this handle*
    /// uses, or clears it with `None`.
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
    /// session: another view taken from the same [`Session`] keeps its own
    /// value. It applies to every request the handle submits — commands,
    /// inquiries, and operations, including the ones the noun accessors submit
    /// — with exactly one exception.
    ///
    /// # Urgent requests are never demoted
    ///
    /// A request the crate classifies [`ControlClass::Urgent`] — the typed
    /// stops [`PanTiltStop`], [`ZoomStop`], [`FocusStop`], and
    /// [`CommandCancel`](crate::request::builtin::CommandCancel) — ignores
    /// this default and stays urgent. A handle demoted to
    /// [`ControlClass::Background`] for telemetry polling therefore still
    /// preempts with an emergency stop. Only
    /// [`execute_with_class`](Self::execute_with_class),
    /// [`inquire_with_class`](Self::inquire_with_class), and
    /// [`submit_with_class`](Self::submit_with_class) can demote an urgent
    /// request, and they do so for that one submission only.
    ///
    /// Owner-internal traffic that no caller submitted — the settlement
    /// polling behind [`Operation::settled`] and the observation inquiries
    /// behind `motion()` — keeps its own built-in class.
    pub fn set_command_class(&mut self, class: Option<ControlClass>) {
        self.core.set_command_class(class);
    }

    /// Executes a plain command through this camera's shared owner.
    pub fn execute<C>(&self, command: &C) -> Result<(), Error>
    where
        C: PlainCommand + ?Sized,
    {
        self.core.execute(command)
    }

    /// Executes a plain command in an explicitly named scheduling lane.
    ///
    /// `class` replaces both the command's own [`ControlClass`] and this
    /// handle's [`set_command_class`](Self::set_command_class) default, for
    /// this submission only.
    ///
    /// Unlike the handle default, this **can demote an urgent request**:
    /// naming [`ControlClass::Background`] for a stop or a cancel queues it
    /// behind ordinary control traffic. Nothing in the crate does that for
    /// you; it is only reachable by asking for it here.
    pub fn execute_with_class<C>(&self, command: &C, class: ControlClass) -> Result<(), Error>
    where
        C: PlainCommand + ?Sized,
    {
        self.core.execute_with_class(command, class)
    }

    /// Sends an inquiry and decodes its response through the shared owner.
    pub fn inquire<Q>(&self, inquiry: &Q) -> Result<Q::Response, Error>
    where
        Q: Inquiry + ?Sized,
    {
        self.core.inquire(inquiry)
    }

    /// Sends an inquiry in an explicitly named scheduling lane.
    ///
    /// `class` replaces both the inquiry's own [`ControlClass`] and this
    /// handle's [`set_command_class`](Self::set_command_class) default, for
    /// this submission only.
    pub fn inquire_with_class<Q>(
        &self,
        inquiry: &Q,
        class: ControlClass,
    ) -> Result<Q::Response, Error>
    where
        Q: Inquiry + ?Sized,
    {
        self.core.inquire_with_class(inquiry, class)
    }

    /// Submits a typed operation and returns its borrowed owner-backed handle.
    pub fn submit<K, O>(&self, operation: &O) -> Result<Operation<'session, K>, Error>
    where
        K: completion::Kind,
        O: OperationCommand<K> + ?Sized,
    {
        self.core.submit(operation)
    }

    /// Submits a typed operation in an explicitly named scheduling lane.
    ///
    /// `class` replaces both the operation's own [`ControlClass`] and this
    /// handle's [`set_command_class`](Self::set_command_class) default, for
    /// this submission only. As with
    /// [`execute_with_class`](Self::execute_with_class), this is the one route
    /// that can demote an urgent stop.
    pub fn submit_with_class<K, O>(
        &self,
        operation: &O,
        class: ControlClass,
    ) -> Result<Operation<'session, K>, Error>
    where
        K: completion::Kind,
        O: OperationCommand<K> + ?Sized,
    {
        self.core.submit_with_class::<K, O>(operation, class)
    }
}

#[path = "blocking_nouns.rs"]
mod blocking_nouns;

pub use blocking_nouns::{
    AdvancedAccessor, ExposureAccessor, FocusAccessor, ImageAccessor, MenuAccessor, MotionAccessor,
    MotionSyncAccessor, NdFilterAccessor, PanTiltAccessor, PowerAccessor, PresetsAccessor,
    SystemAccessor, TallyAccessor, WhiteBalanceAccessor, ZoomAccessor,
};

fn ensure_before_deadline(
    control: &BlockingReceiptControl<'_>,
    deadline: std::time::Instant,
) -> Result<(), Error> {
    if control.now() >= deadline {
        Err(Error::Timeout)
    } else {
        Ok(())
    }
}

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

    /// The owning `Session` is the host every camera view borrows from: two
    /// views taken from the same session drive the one owned transport, and an
    /// operation submitted through either of them settles rather than merely
    /// being constructed.
    #[test]
    fn owning_blocking_session_builds_shared_handle_host() {
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
}
