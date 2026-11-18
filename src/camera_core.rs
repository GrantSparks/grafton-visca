//! Native target views generated from one preparation and forwarding contract.
//!
//! Each facade supplies its owner handle and the `async`/`.await` tokens. The
//! blocking expansion calls its worker directly, without an executor or future.
//! `Camera` and `Operation` resolve to the native types in the invoking module.

macro_rules! camera_core {
    (
        core: $core:ident,
        owner: $owner:ty,
        async: [$($async:tt)*],
        await: [$($await:tt)*] $(,)?
    ) => {
        /// Erased owner-backed target shared by typed and dynamic camera views.
        #[derive(Clone)]
        pub(crate) struct $core {
            owner: $owner,
            registration: std::sync::Arc<crate::TargetRegistration>,
            class: crate::prepared::ClassSelection,
        }

        impl std::fmt::Debug for $core {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter
                    .debug_struct("Camera")
                    .field("target", &self.target())
                    .field("profile", &self.profile())
                    .finish_non_exhaustive()
            }
        }

        impl $core {
            /// Returns this view's fixed individual target.
            pub(crate) fn target(&self) -> crate::IndividualCameraId {
                self.registration.target()
            }

            /// Returns this view's validated profile facts.
            pub(crate) fn profile(&self) -> &crate::ProfileSpec {
                self.registration.profile()
            }

            /// Returns the target's profile and coordinate context.
            pub(crate) fn registration(&self) -> &crate::TargetRegistration {
                &self.registration
            }

            /// Returns a cheap live view of this target's write-only state.
            pub(crate) fn state_cache(&self) -> crate::StateCache {
                self.owner.state_cache(self.registration.slot())
            }

            /// Reads live validated tuning once for each preparation, including
            /// updates made after this view was created or cloned (#631).
            fn tuning(&self) -> crate::session_config::ValidatedTuning {
                self.owner.validated_tuning()
            }

            /// Returns this view's default submission class.
            pub(crate) const fn submission_class(&self) -> Option<crate::SubmissionClass> {
                self.class.handle_default()
            }

            /// Sets this view's default submission class.
            pub(crate) fn set_submission_class(&mut self, class: Option<crate::SubmissionClass>) {
                self.class = crate::prepared::ClassSelection::from_handle_default(class);
            }

            /// Prepares and executes a plain command through this target's owner.
            /// A raw no-reply command succeeds on its local write rather than
            /// claiming confirmed camera application.
            pub(crate) $($async)* fn execute<C>(&self, command: &C) -> crate::Result<()>
            where
                C: crate::PlainCommand + ?Sized,
            {
                let prepared = crate::prepared::prepare_command(
                    command,
                    self.registration.as_ref(),
                    self.tuning(),
                    self.class,
                )?;
                let receipt = self.owner.submit_command(prepared) $($await)* ?;
                receipt.wait() $($await)*
            }

            /// Prepares and decodes one typed inquiry through this target's owner.
            pub(crate) $($async)* fn inquire<Q>(&self, inquiry: &Q) -> crate::Result<Q::Response>
            where
                Q: crate::Inquiry + ?Sized,
            {
                self.inquire_with_decoder(inquiry, |profile| inquiry.decoder_for_profile(profile)) $($await)*
            }

            /// Prepares a noun inquiry using its profile-bound response type.
            pub(crate) $($async)* fn inquire_with_decoder<Q, R>(
                &self,
                inquiry: &Q,
                bind_decoder: impl FnOnce(&crate::ProfileSpec) -> crate::Result<crate::ResponseDecoder<R>>,
            ) -> crate::Result<R>
            where
                Q: crate::Inquiry + ?Sized,
            {
                let prepared = crate::prepared::prepare_inquiry_with_decoder(
                    inquiry,
                    self.registration.as_ref(),
                    self.tuning(),
                    self.class,
                    bind_decoder,
                )?;
                let receipt = self.owner.submit_inquiry(prepared) $($await)* ?;
                receipt.wait() $($await)*
            }

            /// Admits an operation and returns its lifecycle handle. Transport
            /// write failures remain part of the operation's eventual outcome.
            pub(crate) $($async)* fn submit<K, O>(&self, operation: &O) -> crate::Result<Operation<K>>
            where
                K: crate::completion::Kind,
                O: crate::OperationCommand<K> + ?Sized,
            {
                let prepared = crate::prepared::prepare_operation::<K, _>(
                    operation,
                    self.registration.as_ref(),
                    self.tuning(),
                    self.class,
                )?;
                let receipt = self.owner.submit_operation(prepared) $($await)* ?;
                Ok(Operation::from_receipt(receipt))
            }

            /// Fences older motion and admits each supported STOP independently
            /// under one deadline, retaining every axis's outcome in the report.
            pub(crate) $($async)* fn stop_all_motion(&self) -> crate::Result<crate::HaltReport> {
                let started = self.owner.now();
                let prepared = crate::prepared::prepare_halt(self.registration.as_ref(), self.tuning());
                self.owner.halt(prepared, started) $($await)*
            }

            pub(crate) $($async)* fn is_moving(&self, query: crate::camera::MotionQuery) -> crate::Result<bool> {
                let window = crate::prepared::MotionWindow::new(query)?;
                let queries = crate::prepared::prepare_position_queries(
                    self.registration.as_ref(), self.tuning(), query.axes,
                )?;
                self.owner.observe_motion(window, &queries) $($await)*
            }

            /// Observes exactly the selected axes under one absolute deadline
            /// covering inquiry admission, replies and polling intervals.
            pub(crate) $($async)* fn wait_until_idle(&self, wait: crate::camera::IdleWait) -> crate::Result<()> {
                wait.validate()?;
                let queries = crate::prepared::prepare_position_queries(
                    self.registration.as_ref(), self.tuning(), wait.axes(),
                )?;
                self.owner.wait_until_idle(wait, &queries) $($await)*
            }
        }
    };
}

pub(crate) use camera_core;
