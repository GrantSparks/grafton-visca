//! Native operation handles generated from one lifecycle contract.
//!
//! The receipt and `async`/`.await` tokens select the native owner path. Shared
//! lifecycle documentation and forwarding stay in one definition.

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
        /// The completion marker `K` is the only type parameter. No profile,
        /// transport, executor, or runtime type appears in this public handle, and it
        /// borrows nothing: it keeps the session's owner alive, so it may outlive the
        /// `Session` and `Camera` it came from and move to another thread. The
        /// owner remains authoritative for protocol lifecycle, timeouts, settlement,
        /// and cancellation policy; the handle observes it.
        ///
        #[doc = $wait_doc]
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
        /// running until something ends it.
        ///
        /// To bound movement by a scope, write a small guard whose own `Drop` submits
        /// the typed STOP — see the guard pattern in `docs/migration_2_0.md` and
        /// the operation-handle examples. For an explicit stop on a path you
        /// control, use `camera.pan_tilt().stop()`, `camera.zoom().stop()`,
        /// `camera.focus().stop()`, or `camera.motion().stop_all()`;
        /// [`cancel`](Self::cancel) is protocol cancellation and does not by itself
        /// prove motion ended.
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
            /// Creates a public handle over an admitted owner receipt.
            ///
            /// This constructor is crate-private: only the typed admission path may
            /// create a lifecycle handle, after the owner admitted the request.
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

            /// Cancels the operation and waits for the cancellation's conclusion,
            /// bounded by the configured cancellation deadline.
            ///
            /// The result is [`crate::operation::CancellationOutcome::Cancelled`] when cancellation
            /// won, [`crate::operation::CancellationOutcome::Completed`] when the operation was applied
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
            /// standard VISCA socket-cancel command; without it — the built-in
            /// PTZOptics profiles ([`crate::profiles::PtzOpticsG2`], [`crate::profiles::PtzOpticsG3`] and
            /// [`crate::profiles::PtzOptics30x`]) are in that position — the owner refuses with
            /// [`Error::CancellationNotSupported`] and leaves the original request scheduled,
            /// retryable, and able to complete. The handle is unaffected and can
            /// still wait for it. A refusal does not stop the camera; a moving axis
            /// ends with an applied typed STOP.
            ///
            /// A cancellation that was accepted but then failed without ending the
            /// operation (its cancel write failed, or its observation deadline
            /// expired) returns that error; the handle still observes the operation's
            /// own outcome.
            ///
            /// [`crate::profiles::PtzOpticsG2`]: crate::profiles::PtzOpticsG2
            /// [`crate::profiles::PtzOpticsG3`]: crate::profiles::PtzOpticsG3
            /// [`crate::profiles::PtzOptics30x`]: crate::profiles::PtzOptics30x
            pub $($async)* fn cancel(&mut self) -> Result<CancellationOutcome, Error> {
                self.receipt.cancel(None) $($await)*
            }

            /// [`cancel`](Self::cancel), bounded by `timeout` instead of the
            /// configured cancellation deadline. If it expires, the cancellation
            /// intent stays recorded and a later `cancel` observes it.
            pub $($async)* fn cancel_with_timeout(&mut self, timeout: Duration) -> Result<CancellationOutcome, Error> {
                self.receipt.cancel(Some(timeout)) $($await)*
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
            /// settlement condition. The default budget is the larger of the configured
            /// settlement override and this request's retry budget. Use
            /// [`settled_with_timeout`](Self::settled_with_timeout) for an exact bound.
            ///
            /// Application is cached, so calling this after
            /// [`applied`](Operation::applied) continues from it. Polled settlement proves observed stability, not arrival at the requested endpoint.
            /// Later conflicting admission supersedes unfinished polled evidence. Settlement proven
            /// by position polling is cached once proven; a polling wait that is
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
