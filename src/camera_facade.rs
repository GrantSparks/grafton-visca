//! Native public camera facades generated from one forwarding contract.
//!
//! Mode-specific constructors stay in their facade modules. The core, owner
//! handle and `async`/`.await` fragments select their native implementation;
//! ownership, admission and forwarding documentation is shared here. The
//! handwritten camera constructors are inserted in the shared impl so public
//! API listings retain their existing inherent-impl grouping.

// Shared contracts expand to literals so both handwritten methods retain
// the same documentation while their mode-specific release details stay local.
macro_rules! session_contract {
    (close) => {
        r#"Requests owner shutdown and consumes this session, waiting for teardown.

Unlike [`Self::shutdown`], this is a deterministic transport teardown barrier:
it completes only after the owner has dropped its transport, so reopening the
same endpoint after `close` is safe. If shutdown was accepted, the terminal
owner result is returned after teardown: explicit shutdown returns `Ok(())`,
while a transport close or stream poison that won the race is returned
unchanged. If sending this call's shutdown signal fails immediately, that error
is preserved. An owner that panicked is reported as [`crate::Error::OwnerTerminated`]."#
    };
    (diagnostics) => {
        r#"Subscribes to the owner's bounded, best-effort diagnostic stream.

Use [`crate::observability::DiagnosticCapacity`] to select a queue size from
1 to 128 events. One owner retains up to four live subscriptions. A slow
subscriber never stalls command or transport processing.
Its dropped events are visible in [`Self::metrics`].

Delivery is not ordered against receipt resolution: a wait on an operation can
complete before that operation's terminal event is queued here. Once
[`Self::close`] completes, every retained event the owner published is queued."#
    };
}
pub(crate) use session_contract;

macro_rules! camera_facade {
    (
        core: $core:ident,
        owner: $owner:ty,
        async: [$($async:tt)*],
        await: [$($await:tt)*],
        error: [$($error:tt)*],
        session_mode_doc: $session_mode_doc:literal,
        camera_session_constructors: {$($constructors:tt)*} $(,)?
    ) => {
        /// One owner session and its transport.
        ///
        /// The session and its [`Camera`] views are `Clone + Send + Sync`. An
        /// [`Operation`] handle is `Send + Sync`, borrows nothing, and can move to
        /// another thread; its waits borrow that handle. The owner serializes every
        /// call while timers, reads, and correlation cleanup continue independently
        /// of any caller waiting on an operation.
        ///
        /// Every clone shares the one owner. Dropping the last handle — session,
        /// camera, or operation — stops it and releases the transport without sending
        /// a protocol STOP; [`close`](Self::close) is the explicit teardown barrier.
        ///
        #[doc = $session_mode_doc]
        #[derive(Clone)]
        pub struct Session {
            owner: $owner,
            config: std::sync::Arc<crate::session_config::ValidatedSessionConfig>,
        }

        impl std::fmt::Debug for Session {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter
                    .debug_struct("Session")
                    .field(
                        "targets",
                        &std::fmt::from_fn(|list| {
                            list.debug_list().entries(self.config.targets()).finish()
                        }),
                    )
                    .finish_non_exhaustive()
            }
        }

        impl Session {
            /// Returns a camera view for the registered target.
            ///
            /// Typed profiles require exact equality with their validated profile facts;
            /// [`crate::profiles::AnyProfile`] accepts the registered runtime profile.
            pub fn camera<P: CameraProfile>(
                &self,
                target: crate::IndividualCameraId,
            ) -> Result<Camera<P> $($error)*> {
                let target = TargetSlot::from(target);
                let registration = self.config.registered_target(target)?;
                P::validate_spec(registration.profile())?;
                Ok(Camera::from_core(self.core(registration)))
            }

            /// The erased view of `target`, registered with `profile`.
            fn core(&self, registration: Arc<crate::TargetRegistration>) -> $core {
                $core {
                    owner: self.owner.clone(),
                    registration,
                    class: ClassSelection::Request,
                }
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
            /// **Every request prepared after this update completes uses the new values**
            /// — its acknowledgement, completion, settlement, and inquiry deadlines,
            /// its retry budget, and its pacing floor. The owner's session-wide pacing
            /// and per-target command-socket capacity are re-derived immediately, so
            /// work still queued behind pacing is released under the new values too.
            ///
            /// **Requests already in flight keep the deadlines they were admitted
            /// with.** 2.0 stamps a request's deadlines once, at preparation, and the
            /// engine derives its absolute phase deadlines from that stamp; nothing is
            /// re-timed underneath an [`Operation`] a caller is already holding. This
            /// is deliberate: a deadline that moved after submission would change the
            /// contract of work a caller has already observed, so tuning applies only
            /// to requests prepared after the update.
            ///
            /// The update is not a merge: a field left unset returns to its profile
            /// default rather than keeping the value a previous call installed.
            /// Construction-only recovery policy lives on [`crate::config::SessionConfig`], outside
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
            pub $($async)* fn set_tuning(&self, tuning: OperationalTuning) -> Result<() $($error)*> {
                let validated_tuning = self.config.validate_tuning(tuning);
                self.owner.reconfigure(validated_tuning) $($await)*
            }

            /// Requests owner shutdown.
            ///
            /// This is an idempotent shutdown request. It returns once the request is
            /// accepted by the owner's bounded shutdown boundary; it does not wait for
            /// transport teardown. After this call,
            /// new request admission is rejected with [`crate::Error::RuntimeShutdown`]. All
            /// [`Session`] clones share this one shutdown boundary.
            pub fn shutdown(&self) -> Result<() $($error)*> {
                self.owner.shutdown()
            }

            /// Returns a scalar snapshot of the owner without cloning diagnostics.
            ///
            /// After normal conclusion, returns the retained terminal snapshot. An
            /// owner that unwinds before conclusion reports `OwnerTerminated`.
            pub $($async)* fn metrics(&self) -> Result<MetricsSnapshot $($error)*> {
                self.owner.metrics() $($await)*
            }
        }

        /// A profile-aware camera view onto one [`Session`] target.
        ///
        /// The profile parameter supplies compile-time capability gates; owner and
        /// transport state remain in the shared erased core. The view is
        /// `Clone + Send + Sync` and borrows nothing: it keeps the session's owner
        /// alive, and concurrent calls through it are serialized by the owner.
        #[must_use]
        pub struct Camera<P: CameraProfile = AnyProfile> {
            core: $core,
            _profile: PhantomData<fn() -> P>,
        }

        impl<P: CameraProfile> Clone for Camera<P> {
            fn clone(&self) -> Self {
                Self {
                    core: self.core.clone(),
                    _profile: PhantomData,
                }
            }
        }

        impl<P: CameraProfile> fmt::Debug for Camera<P> {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct("Camera")
                    .field("target", &self.core.target())
                    .field("profile", &self.core.profile())
                    .finish_non_exhaustive()
            }
        }

        impl<P: CameraProfile> Camera<P> {
            pub(crate) fn from_core(core: $core) -> Self {
                Self {
                    core,
                    _profile: PhantomData,
                }
            }

            pub(crate) const fn core(&self) -> &$core {
                &self.core
            }

            crate::camera_view::camera_view_getters!();

            /// Erases only the compile-time profile parameter, retaining this owner and target.
            pub fn into_any(self) -> Camera<AnyProfile> {
                Camera::from_core(self.core)
            }

            /// Checks the complete registered profile before selecting a typed view.
            pub fn into_typed<T: CompileTimeProfile>(self) -> Result<Camera<T>> {
                T::validate_spec(self.core.profile())?;
                Ok(Camera::from_core(self.core))
            }

            /// Executes a plain command through this camera's shared owner.
            ///
            /// This intentionally has no request-specific `Permits` bound. Noun and
            /// accessor methods use sealed `Permits<gate::X>` bounds: evidenced typed
            /// profiles and `AnyProfile` permit calls, while preparation checks the
            /// registered profile before I/O. `Has*` markers remain evidence claims;
            /// this direct method is the uniform typed/downstream extension boundary.
            ///
            /// Preparation calls [`crate::request::Request::validate_for_target`] before
            /// encoding, owner admission, or transport I/O. Crate-provided requests
            /// submitted directly return [`crate::Error::FeatureNotSupported`] there
            /// when the selected profile lacks their required feature. Raw and other
            /// downstream requests are explicit low-level extensions and may provide
            /// their own profile validation.
            ///
            /// Ordinary commands wait for their terminal protocol application. A raw
            /// [`crate::raw::RawReplyShape::NoReply`] command instead succeeds once its
            /// local transport write succeeds; it does not claim camera application.
            pub $($async)* fn execute<C>(&self, command: &C) -> Result<() $($error)*>
            where
                C: PlainCommand + ?Sized,
            {
                self.core.execute(command) $($await)*
            }

            /// Sends an inquiry and decodes its response through the shared owner.
            ///
            /// This intentionally has no request-specific `Permits` bound. Noun and
            /// accessor methods use sealed `Permits<gate::X>` bounds: evidenced typed
            /// profiles and `AnyProfile` permit calls, while preparation checks the
            /// registered profile before I/O. `Has*` markers remain evidence claims;
            /// this direct method is the uniform typed/downstream extension boundary.
            ///
            /// Preparation calls [`crate::request::Request::validate_for_target`] before
            /// encoding, owner admission, or transport I/O. Crate-provided inquiries
            /// submitted directly return [`crate::Error::FeatureNotSupported`] there
            /// when the selected profile lacks their required feature. Raw and other
            /// downstream requests are explicit low-level extensions and may provide
            /// their own profile validation.
            pub $($async)* fn inquire<Q>(&self, inquiry: &Q) -> Result<Q::Response $($error)*>
            where
                Q: Inquiry + ?Sized,
            {
                self.core.inquire(inquiry) $($await)*
            }

            /// Submits a typed operation and returns its owner-backed handle once the
            /// owner has admitted it.
            ///
            /// On the async facade, dropping a submission future before the
            /// owner admits its request prevents that request from being written.
            /// Admission and its reply happen in one synchronous owner turn;
            /// dropping after that admission is observation-only detachment,
            /// even if the caller has not yet received the returned handle.
            pub $($async)* fn submit<K, O>(&self, operation: &O) -> Result<Operation<K> $($error)*>
            where
                K: completion::Kind,
                O: OperationCommand<K> + ?Sized,
            {
                self.core.submit(operation) $($await)*
            }
        }

        /// One owner session with one typed or runtime-profile camera.
        ///
        /// This is the single-target counterpart of [`Session`]. The profile is named
        /// once, at construction, and the [`Camera`] this type hands out was built from
        /// that same profile parameter: the session path's second, runtime-checked
        /// profile naming (`session.camera::<P>(target)?`) has no equivalent here, so a
        /// profile mismatch is not expressible.
        ///
        /// The value owns its session, so it is self-sufficient: the connection lives
        /// as long as this camera session (or a [`Camera`] taken out of it) is alive.
        /// Dropping the final owner handle stops the owner and releases the
        /// transport, but does not emit a protocol STOP. Use [`close`](Self::close)
        /// for the explicit teardown that mirrors [`Session::close`].
        pub struct CameraSession<P: CameraProfile = AnyProfile> {
            session: Session,
            camera: Camera<P>,
        }

        impl<P: CameraProfile> fmt::Debug for CameraSession<P> {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct("CameraSession")
                    .field("target", &self.camera.target())
                    .field("profile", &self.camera.profile())
                    .finish_non_exhaustive()
            }
        }

        impl<P: CameraProfile> CameraSession<P> {
            /// Binds the sole registration already proven by camera configuration.
            pub(crate) fn from_registration(
                session: Session,
                registration: std::sync::Arc<crate::TargetRegistration>,
            ) -> Self {
                let camera = Camera::from_core(session.core(registration));
                Self { session, camera }
            }

            $($constructors)*

            /// Erases only the compile-time profile parameter, retaining the same owner.
            #[must_use]
            pub fn into_any(self) -> CameraSession<AnyProfile> {
                CameraSession {
                    session: self.session,
                    camera: self.camera.into_any(),
                }
            }

            /// Checks the complete registered profile before selecting a typed view.
            pub fn into_typed<T: CompileTimeProfile>(self) -> Result<CameraSession<T>> {
                Ok(CameraSession {
                    session: self.session,
                    camera: self.camera.into_typed::<T>()?,
                })
            }

            /// Returns this session's profile-bound camera view.
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
            pub fn target(&self) -> crate::IndividualCameraId {
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
            pub $($async)* fn set_tuning(&self, tuning: OperationalTuning) -> Result<() $($error)*> {
                self.session.set_tuning(tuning) $($await)*
            }

            /// Requests owner shutdown without consuming this value.
            ///
            /// This is the idempotent, non-joining signal; use [`Self::close`] when
            /// the owner and owned transport must be fully released before continuing.
            pub fn shutdown(&self) -> Result<() $($error)*> {
                self.session.shutdown()
            }

            /// Requests owner shutdown and consumes this camera session, waiting for
            /// the owner and its owned transport to be fully released.
            ///
            /// This is the deterministic teardown barrier described by
            /// [`Session::close`]; use [`Self::shutdown`] when only an idempotent,
            /// non-joining signal is required.
            pub $($async)* fn close(self) -> Result<() $($error)*> {
                self.session.close() $($await)*
            }
        }

    };
}

pub(crate) use camera_facade;
