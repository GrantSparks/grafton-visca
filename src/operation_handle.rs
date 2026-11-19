//! Operation handles shared by the blocking and async APIs.

macro_rules! operation_handle {
    (
        receipt: $receipt:ident,
        async: [$($async:tt)*],
        await: [$($await:tt)*],
        description: $description:literal,
        wait_doc: $wait_doc:literal,
        must_use: $must_use:literal $(,)?
    ) => {
        #[doc = $description]
        ///
        /// The completion marker `K` determines which waits are available. The
        /// handle keeps the session owner alive and can move to another thread
        /// or outlive the `Session` and `Camera` that created it.
        ///
        #[doc = $wait_doc]
        ///
        /// # Waits borrow the handle
        ///
        /// Each wait takes `&mut self`. The handle caches confirmed outcomes.
        /// On a targeted move, [`settled`](Operation::settled) can follow
        /// [`applied`](Self::applied). After an observation timeout the
        /// operation keeps running and the handle can wait again.
        ///
        /// # Dropping never stops the camera
        ///
        /// Dropping the handle [detaches](Self::detach) observation. A drive
        /// can continue after an early return or panic. End continuous motion
        /// with the axis's `stop()` or `camera.motion().stop_all()`. To attempt
        /// a STOP on scope exit, see the [guard example](https://github.com/GrantSparks/grafton-visca/blob/main/examples/operation_handles.rs).
        #[must_use = $must_use]
        pub struct Operation<K>
        where
            K: completion::Kind,
        {
            receipt: $receipt<K>,
        }

        impl<K> Operation<K>
        where
            K: completion::Kind,
        {
            pub(crate) fn from_receipt(receipt: $receipt<K>) -> Self {
                Self { receipt }
            }

            /// Returns the opaque identity assigned when this operation was admitted.
            #[must_use]
            pub fn id(&self) -> OperationId {
                OperationId::from_nonzero(self.receipt.id())
            }

            /// Waits for exact protocol application, bounded by the configured
            /// observer deadline.
            pub $($async)* fn applied(&mut self) -> Result<(), Error> {
                self.receipt.applied(None) $($await)*
            }

            /// Waits for exact protocol application, bounded by `timeout`.
            ///
            /// The deadline bounds only this wait. If it expires the wait returns
            /// [`Error::ObservationTimeout`]: the operation keeps running, its
            /// scheduler deadline is unchanged, and the handle can wait again.
            pub $($async)* fn applied_with_timeout(&mut self, timeout: Duration) -> Result<(), Error> {
                self.receipt.applied(Some(timeout)) $($await)*
            }

            /// Requests cancellation and waits up to the configured cancellation deadline.
            /// Returns [`CancellationOutcome::Cancelled`] on confirmed cancellation,
            /// [`CancellationOutcome::Completed`] if the operation applied first,
            /// or the operation's error if it failed first. Repeated calls observe
            /// the same cancellation, including after a wait times out.
            ///
            /// Queued work can be cancelled locally. After transmission the camera
            /// must support VISCA socket cancellation. On a profile such as
            /// [`crate::profiles::PtzOpticsG2`], [`Error::CancellationNotSupported`]
            /// leaves the request scheduled and eligible for retries. The handle
            /// can still observe it after a refusal or a failed cancel write.
            /// Send an explicit STOP to end continuous movement.
            pub $($async)* fn cancel(&mut self) -> Result<CancellationOutcome, Error> {
                self.receipt.cancel(None) $($await)*
            }

            /// [`cancel`](Self::cancel), bounded by `timeout` instead of the
            /// configured cancellation deadline. If it expires, the cancellation
            /// intent stays recorded and a later `cancel` observes it.
            pub $($async)* fn cancel_with_timeout(&mut self, timeout: Duration) -> Result<CancellationOutcome, Error> {
                self.receipt.cancel(Some(timeout)) $($await)*
            }

            /// Releases the handle and leaves the submitted operation running.
            pub fn detach(self) {}
        }

        impl Operation<completion::Targeted> {
            /// Waits for exact application and the profile-selected protocol
            /// settlement condition. The default budget is the larger of the configured
            /// settlement override and this request's retry budget. Use
            /// [`settled_with_timeout`](Self::settled_with_timeout) for an exact bound.
            ///
            /// Application is cached, so calling this after
            /// [`applied`](Operation::applied) continues from it. Polled settlement proves observed stability, not arrival at the requested endpoint.
            /// A later overlapping admission or an older overlapping write after
            /// this operation writes supersedes unfinished polled evidence. Settlement
            /// proven by position polling is cached. A polling wait that is
            /// abandoned or times out restarts with a fresh proof.
            pub $($async)* fn settled(&mut self) -> Result<crate::Settlement, Error> {
                self.receipt.settled(None) $($await)*
            }

            /// [`settled`](Self::settled), bounded by `timeout`.
            ///
            /// The deadline bounds only this wait; the prepared scheduler and
            /// settlement policy are unchanged. Each abandoned polling attempt may
            /// leave its admitted position inquiries running until their own
            /// deadlines, within the session's admission capacity.
            pub $($async)* fn settled_with_timeout(&mut self, timeout: Duration) -> Result<crate::Settlement, Error> {
                self.receipt.settled(Some(timeout)) $($await)*
            }
        }

        impl<K> std::fmt::Debug for Operation<K>
        where
            K: completion::Kind,
        {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.receipt.fmt_public_handle(formatter)
            }
        }

    };
}

pub(crate) use operation_handle;
