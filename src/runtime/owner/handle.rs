//! The owner-handle methods and receipt waits, written once for both shells
//! (#801, #802).
//!
//! The blocking worker and the async actor expose the same handle contract
//! over the same boundary lanes; they differ only in how a caller waits. Each
//! shell therefore implements a handful of waiting primitives on its handle,
//! and [`owner_handle_methods!`] expands every method built on them, once with
//! `async`/`.await` and once without. The `blocking` facade stays
//! executor-free: its expansion contains no future.
//!
//! The primitives a handle provides, all measured on its own clock:
//!
//! - `now()`: the owner clock.
//! - `sleep(duration)`.
//! - `send_lane(lane, message)` and `send_lane_until(lane, message, deadline)`:
//!   deliver one boundary message; the bounded form gives up (`None`) once
//!   `deadline` passes.
//! - `boundary_reply(reply)` and `boundary_reply_until(reply, deadline)`: the
//!   boundary's answer, or the owner's terminal error once it is gone (#626).
//! - `next_observation(terminal, cancellation, deadline)`: the one race every
//!   receipt wait uses.

/// Expands the shared owner-handle methods and receipt waits for one shell.
///
/// `handle` is the shell's handle type, `async` is `[async]` or `[]`, `await`
/// is `[.await]` or `[]`, and `block` is `[async move]` or `[]`, prefixed to
/// the closures passed to the prepared-request admission seams.
macro_rules! owner_handle_methods {
    (
        handle: $handle:ty,
        async: [$($async:tt)*],
        await: [$($await:tt)*],
        block: [$($block:tt)*] $(,)?
    ) => {
        impl $handle {
            /// The absolute observer deadline `timeout` after now.
            pub(crate) fn deadline_after(
                &self,
                timeout: std::time::Duration,
            ) -> Result<std::time::Instant, crate::Error> {
                super::receipt::observer_deadline(self.now(), timeout)
            }

            /// Rechecks the owner clock at every admission and sample
            /// boundary. Equality with the deadline is already too late for
            /// another inquiry to be enqueued.
            fn ensure_before(&self, deadline: std::time::Instant) -> Result<(), crate::Error> {
                if self.now() >= deadline {
                    Err(crate::Error::query_timeout())
                } else {
                    Ok(())
                }
            }

            /// Send one control request and wait for its answer.
            $($async)* fn control_request<T>(
                &self,
                request: impl FnOnce(flume::Sender<T>) -> super::boundary::ControlBoundary,
            ) -> Result<T, crate::Error>
            where
                T: Send,
            {
                let (reply, receiver) = flume::bounded(1);
                self.send_lane(&self.core.control, request(reply)) $($await)* ?;
                self.boundary_reply(&receiver) $($await)*
            }

            /// Class-specific typed admission seam for ordinary commands.
            pub(crate) $($async)* fn submit_command(
                &self,
                prepared: crate::prepared::PreparedCommand,
            ) -> Result<super::receipt::CommandReceipt<Self>, crate::Error> {
                prepared
                    .admit_with(|request, timeout| $($block)* {
                        self.submit_with_timeout(request, timeout) $($await)*
                            .map(|core| super::receipt::CommandReceipt {
                                core,
                                owner: self.clone(),
                            })
                    })
                    $($await)*
            }

            /// Class-specific typed admission seam retaining the external
            /// decoder.
            pub(crate) $($async)* fn submit_inquiry<R>(
                &self,
                prepared: crate::prepared::PreparedInquiry<R>,
            ) -> Result<super::receipt::InquiryReceipt<R, Self>, crate::Error> {
                prepared
                    .admit_with(|request, decoder, timeout| $($block)* {
                        self.submit_with_timeout(request, timeout) $($await)*
                            .map(|core| super::receipt::InquiryReceipt {
                                core,
                                decoder,
                                owner: self.clone(),
                            })
                    })
                    $($await)*
            }

            $($async)* fn submit_inquiry_until<R>(
                &self,
                prepared: crate::prepared::PreparedInquiry<R>,
                deadline: std::time::Instant,
            ) -> Result<super::receipt::InquiryReceipt<R, Self>, crate::Error> {
                prepared
                    .admit_with(|request, decoder, timeout| $($block)* {
                        self.submit_with_timeout_until(request, timeout, deadline) $($await)*
                            .map(|core| super::receipt::InquiryReceipt {
                                core,
                                decoder,
                                owner: self.clone(),
                            })
                    })
                    $($await)*
            }

            /// Class-specific typed admission seam retaining operation
            /// semantics.
            pub(crate) $($async)* fn submit_operation<K>(
                &self,
                prepared: crate::prepared::PreparedOperation<K>,
            ) -> Result<super::receipt::OperationReceipt<K, Self>, crate::Error>
            where
                K: crate::completion::Kind,
            {
                prepared
                    .admit_with(|request, affected_axes, settlement, timeouts| $($block)* {
                        self.submit_with_timeout(request, timeouts.applied) $($await)*
                            .map(|core| super::receipt::OperationReceipt {
                                observation: super::OperationObservation::new(
                                    core,
                                    timeouts.cancellation,
                                ),
                                affected_axes,
                                settlement,
                                owner: self.clone(),
                            })
                    })
                    $($await)*
            }

            /// Fails immediately when shared boundary/engine capacity is
            /// exhausted, then returns as soon as the owner admits the
            /// request. Admission is the success boundary: a transport write
            /// failure is reported through the receipt's outcome (D24).
            $($async)* fn submit_with_timeout(
                &self,
                request: super::RuntimeRequest,
                configured_timeout: std::time::Duration,
            ) -> Result<super::ReceiptCore, crate::Error> {
                let pending = self.core.admit(request)?;
                let reply = self.boundary_reply(pending.reply()) $($await)*;
                pending.receipt(reply, configured_timeout)
            }

            /// [`Self::submit_with_timeout`] whose admission must happen
            /// before `deadline`. A boundary the owner claimed before its
            /// expiry is authoritative, so its reply is still observed.
            $($async)* fn submit_with_timeout_until(
                &self,
                request: super::RuntimeRequest,
                configured_timeout: std::time::Duration,
                deadline: std::time::Instant,
            ) -> Result<super::ReceiptCore, crate::Error> {
                let pending = self.core.admit_until(request, deadline, self.now())?;
                let reply = match self.boundary_reply_until(pending.reply(), deadline) $($await)* {
                    Some(reply) => reply,
                    None => {
                        pending.expire(&self.core)?;
                        self.boundary_reply(pending.reply()) $($await)*
                    }
                };
                pending.receipt(reply, configured_timeout)
            }

            /// Orders one owner halt: older declared motion is fenced and
            /// every supported STOP is admitted under one deadline measured
            /// from `started`; the report carries each axis's outcome.
            pub(crate) $($async)* fn halt(
                &self,
                prepared: crate::prepared::PreparedHalt,
                started: std::time::Instant,
            ) -> Result<crate::HaltReport, crate::Error> {
                let deadline = super::receipt::observer_deadline(started, prepared.budget)?;
                let validity = super::boundary::AdmissionValidity::until(deadline);
                let (reply, receiver) = flume::bounded(1);
                let boundary =
                    super::boundary::CancellationBoundary::Halt(super::halt::HaltBoundary {
                        prepared,
                        deadline,
                        validity: validity.clone(),
                        reply,
                    });
                let answer = match self
                    .send_lane_until(&self.core.cancellations, boundary, deadline)
                    $($await)*
                {
                    Some(sent) => {
                        sent?;
                        self.boundary_reply_until(&receiver, deadline) $($await)*
                    }
                    None => None,
                };
                let receipt = match answer {
                    Some(answer) => answer??,
                    None => {
                        if validity.expire_before_admission() {
                            return Err(crate::Error::admission_timeout());
                        }
                        self.boundary_reply(&receiver) $($await)* ??
                    }
                };
                let mut outcomes = [
                    crate::HaltOutcome::Unsupported,
                    crate::HaltOutcome::Unsupported,
                    crate::HaltOutcome::Unsupported,
                ];
                for (outcome, slot) in outcomes.iter_mut().zip(receipt.slots) {
                    if let Some(slot) = slot {
                        let applied = match slot {
                            Ok(core) => self
                                .wait_core_until(&core, receipt.deadline)
                                $($await)*
                                .and_then(super::normalize_command_outcome),
                            Err(error) => Err(error),
                        };
                        *outcome = super::halt::outcome(applied);
                    }
                }
                Ok(super::halt::report(outcomes))
            }

            /// Deliver one cancellation request to the owner and return its
            /// answer, all before `deadline` (#777). A full cancellation lane
            /// is backpressure; a closed one is the session's terminal error.
            $($async)* fn request_cancellation(
                &self,
                request: super::CancellationRequest,
                deadline: std::time::Instant,
            ) -> Result<(), crate::Error> {
                let id = request.id;
                let (reply, receiver) = flume::bounded(1);
                let boundary = super::boundary::CancellationBoundary::Cancel { request, reply };
                match self
                    .send_lane_until(&self.core.cancellations, boundary, deadline)
                    $($await)*
                {
                    Some(sent) => sent?,
                    None => return Err(super::observation_timeout(id)),
                }
                match self.boundary_reply_until(&receiver, deadline) $($await)* {
                    Some(answer) => answer?,
                    None => Err(super::observation_timeout(id)),
                }
            }

            /// Reads scalar owner metrics through a dedicated bounded control
            /// request. This path never clones the diagnostic ring.
            pub(crate) $($async)* fn metrics(
                &self,
            ) -> Result<crate::observability::MetricsSnapshot, crate::Error> {
                self.control_request(super::boundary::ControlBoundary::Metrics) $($await)* ?
            }

            pub(crate) $($async)* fn subscribe_diagnostics(
                &self,
                capacity: usize,
            ) -> Result<super::DiagnosticSubscription, crate::Error> {
                self.control_request(|reply| {
                    super::boundary::ControlBoundary::SubscribeDiagnostics { capacity, reply }
                })
                $($await)* ?
            }

            /// Installs new session tuning through the owner's control
            /// boundary (#631).
            ///
            /// The owner applies the update on its own turn, so the write is
            /// ordered against every other boundary message and against the
            /// scheduler itself, and this returns once the owner has applied
            /// it: the next request prepared uses the new values.
            pub(crate) $($async)* fn reconfigure(
                &self,
                validated_tuning: Result<crate::OperationalTuning, crate::Error>,
            ) -> Result<(), crate::Error> {
                self.control_request(|reply| super::boundary::ControlBoundary::Reconfigure {
                    validated_tuning: Box::new(validated_tuning),
                    reply,
                })
                $($await)* ?
            }

            pub(crate) fn state_cache(
                &self,
                target: crate::CameraId,
            ) -> crate::state_cache::StateCache {
                self.core.state_cache(target)
            }

            /// Reads the tuning the owner is currently preparing requests
            /// under.
            pub(crate) fn tuning(&self) -> crate::OperationalTuning {
                self.core.tuning()
            }

            /// Coalesced idempotent shutdown; see
            /// [`super::boundary::OwnerHandleCore::shutdown`].
            pub(crate) fn shutdown(&self) -> Result<(), crate::Error> {
                self.core.shutdown()
            }

            /// Proves the polled axes idle: snapshots every `poll.interval`
            /// until two consecutive ones agree, before one absolute
            /// `deadline` (see [`crate::prepared::SettlementPoll`]). The owner
            /// keeps the session progressing between samples. Errors are the
            /// samples' own; callers map them to their public contract.
            $($async)* fn poll_settlement(
                &self,
                poll: super::receipt::PositionPoll<'_>,
                deadline: std::time::Instant,
            ) -> Result<(), crate::Error> {
                let mut settlement = crate::prepared::SettlementPoll::new(
                    poll.axes,
                    poll.tolerance,
                    poll.interval,
                    deadline,
                );
                loop {
                    let snapshot = self.sample_positions(poll.queries, deadline) $($await)* ?;
                    match settlement.observe(snapshot, self.now())? {
                        crate::prepared::SettlementStep::Settled => return Ok(()),
                        crate::prepared::SettlementStep::SampleAfter(pause) => {
                            self.sleep(pause) $($await)*
                        }
                    }
                }
            }

            /// Waits until the `wait` axes, sampled by `queries`, are idle
            /// before `wait.timeout` elapses.
            pub(crate) $($async)* fn wait_until_idle(
                &self,
                wait: crate::camera::IdleWait,
                queries: &crate::prepared::PositionQueryPlan,
            ) -> Result<(), crate::Error> {
                let deadline = self.deadline_after(wait.timeout)?;
                let poll = super::receipt::PositionPoll {
                    queries,
                    axes: wait.axes,
                    tolerance: wait.tolerance,
                    interval: wait.interval,
                };
                self.poll_settlement(poll, deadline) $($await)*
            }

            /// One movement observation across `window` (#781): a baseline
            /// snapshot, then a final one that starts no earlier than the
            /// window allows, all before the window's budget elapses.
            pub(crate) $($async)* fn observe_motion(
                &self,
                window: crate::prepared::MotionWindow,
                queries: &crate::prepared::PositionQueryPlan,
            ) -> Result<bool, crate::Error> {
                let deadline = self.deadline_after(window.budget()?)?;
                let baseline = self.sample_positions(queries, deadline) $($await)* ?;
                let window = window.observe_baseline(baseline, self.now(), deadline)?;
                loop {
                    let now = self.now();
                    if now >= window.final_not_before() {
                        break;
                    }
                    self.sleep(window.final_not_before().saturating_duration_since(now))
                        $($await)*;
                }
                self.ensure_before(deadline)?;
                let started_at = self.now();
                let last = self.sample_positions(queries, deadline) $($await)* ?;
                window.observe_final(last, started_at)
            }

            /// Samples exactly the prepared position inquiries, in axis
            /// order, before one absolute deadline.
            $($async)* fn sample_positions(
                &self,
                queries: &crate::prepared::PositionQueryPlan,
                deadline: std::time::Instant,
            ) -> Result<crate::prepared::PositionSnapshot, crate::Error> {
                Ok(crate::prepared::PositionSnapshot {
                    pan_tilt: match &queries.pan_tilt {
                        Some(query) => Some(self.sample(query, deadline) $($await)* ?),
                        None => None,
                    },
                    zoom: match &queries.zoom {
                        Some(query) => Some(self.sample(query, deadline) $($await)* ?),
                        None => None,
                    },
                    focus: match &queries.focus {
                        Some(query) => Some(self.sample(query, deadline) $($await)* ?),
                        None => None,
                    },
                    iris: match &queries.iris {
                        Some(query) => Some(self.sample(query, deadline) $($await)* ?),
                        None => None,
                    },
                    nd_filter: match &queries.nd_filter {
                        Some(query) => Some(self.sample(query, deadline) $($await)* ?),
                        None => None,
                    },
                })
            }

            /// One position inquiry, admitted and answered strictly before
            /// `deadline`. Any failure is reported as the query timing out.
            $($async)* fn sample<R>(
                &self,
                query: &crate::prepared::PreparedInquiryTemplate<R>,
                deadline: std::time::Instant,
            ) -> Result<R, crate::Error> {
                self.sample_before(query, deadline)
                    $($await)*
                    .map_err(crate::Error::into_query_timeout)
            }

            $($async)* fn sample_before<R>(
                &self,
                query: &crate::prepared::PreparedInquiryTemplate<R>,
                deadline: std::time::Instant,
            ) -> Result<R, crate::Error> {
                self.ensure_before(deadline)?;
                let receipt = self
                    .submit_inquiry_until(query.instantiate(), deadline)
                    $($await)* ?;
                let value = receipt.wait_until(deadline) $($await)* ?;
                self.ensure_before(deadline)?;
                Ok(value)
            }

            /// Waits until `verdict` can be read from an operation's
            /// observation state; see [`super::receipt::OperationWait`].
            ///
            /// Every value a slot delivers is recorded into the handle's cache
            /// as soon as it is received, so abandoning the wait never loses
            /// one.
            $($async)* fn observe_until<T>(
                &self,
                observation: &mut super::OperationObservation,
                deadline: std::time::Instant,
                verdict: impl Fn(&mut super::OperationObservation, std::time::Instant) -> Option<T>,
            ) -> Result<T, crate::Error> {
                let mut wait = super::receipt::OperationWait::new(observation, verdict, deadline);
                loop {
                    if let Some(value) = wait.verdict() {
                        return Ok(value);
                    }
                    let (terminal, cancellation) = wait.slots();
                    let wake = self
                        .next_observation(terminal, cancellation, deadline)
                        $($await)*;
                    if let std::ops::ControlFlow::Break(verdict) = wait.absorb(wake, &self.core) {
                        return verdict;
                    }
                }
            }

            /// Waits for a command or inquiry receipt's terminal outcome.
            $($async)* fn wait_core_until(
                &self,
                core: &super::ReceiptCore,
                deadline: std::time::Instant,
            ) -> Result<super::RuntimeOutcome, crate::Error> {
                if let Some(concluded) = core.try_conclude(deadline) {
                    return concluded;
                }
                self.next_observation(&core.completion, None, deadline)
                    $($await)*
                    .conclude(core, &self.core, deadline)
            }
        }

        impl super::receipt::CommandReceipt<$handle> {
            /// Waits for the command's outcome within its configured observer
            /// deadline.
            pub(crate) $($async)* fn wait(self) -> Result<(), crate::Error> {
                let deadline = self.owner.deadline_after(self.core.configured_timeout())?;
                self.wait_until(deadline) $($await)*
            }

            $($async)* fn wait_until(self, deadline: std::time::Instant) -> Result<(), crate::Error> {
                self.owner
                    .wait_core_until(&self.core, deadline)
                    $($await)*
                    .and_then(super::normalize_command_outcome)
            }
        }

        impl<T> super::receipt::InquiryReceipt<T, $handle> {
            /// Waits for the decoded reply within the configured observer
            /// deadline.
            pub(crate) $($async)* fn wait(self) -> Result<T, crate::Error> {
                let deadline = self.owner.deadline_after(self.core.configured_timeout())?;
                self.wait_until(deadline) $($await)*
            }

            $($async)* fn wait_until(self, deadline: std::time::Instant) -> Result<T, crate::Error> {
                let outcome = self.owner.wait_core_until(&self.core, deadline) $($await)* ?;
                super::normalize_inquiry_outcome(outcome, &self.decoder)
            }
        }

        impl<K> super::receipt::OperationReceipt<K, $handle>
        where
            K: crate::completion::Kind,
        {
            /// Waits for application, within `timeout` or the configured
            /// observer deadline.
            pub(crate) $($async)* fn applied(
                &mut self,
                timeout: Option<std::time::Duration>,
            ) -> Result<(), crate::Error> {
                let timeout = timeout.unwrap_or_else(|| self.observation.applied_timeout());
                let deadline = self.owner.deadline_after(timeout)?;
                self.applied_until(deadline) $($await)*
            }

            $($async)* fn applied_until(
                &mut self,
                deadline: std::time::Instant,
            ) -> Result<(), crate::Error> {
                self.owner
                    .observe_until(
                        &mut self.observation,
                        deadline,
                        super::OperationObservation::applied,
                    )
                    $($await)* ?
            }

            /// Cancels the operation and waits for the cancellation's
            /// conclusion, within `timeout` or the configured cancellation
            /// deadline (#777).
            ///
            /// A known terminal outcome answers without sending anything.
            /// Otherwise the handle's one cancellation intent is requested; a
            /// request for an intent the owner already holds observes it
            /// instead of sending another. A refusal leaves the operation
            /// running and this receipt intact.
            pub(crate) $($async)* fn cancel(
                &mut self,
                timeout: Option<std::time::Duration>,
            ) -> Result<crate::CancellationOutcome, crate::Error> {
                let timeout = timeout.unwrap_or_else(|| self.observation.cancellation_timeout());
                let deadline = self.owner.deadline_after(timeout)?;
                if let Some(verdict) = self.observation.cancellation(deadline) {
                    return verdict;
                }
                let request = self.observation.cancellation_request();
                if let Err(error) = self.owner.request_cancellation(request, deadline) $($await)* {
                    // A terminal outcome that raced the refusal still decides.
                    return self
                        .observation
                        .cancellation(deadline)
                        .unwrap_or(Err(error));
                }
                self.owner
                    .observe_until(
                        &mut self.observation,
                        deadline,
                        super::OperationObservation::cancellation,
                    )
                    $($await)* ?
            }
        }

        impl super::receipt::OperationReceipt<crate::completion::Targeted, $handle> {
            /// Waits for application and then the profile-selected settlement
            /// condition, within `timeout` or the configured settlement
            /// budget.
            ///
            /// Application is cached, so a wait that is abandoned or times out
            /// during polling restarts from it with a fresh two-sample proof.
            pub(crate) $($async)* fn settled(
                &mut self,
                timeout: Option<std::time::Duration>,
            ) -> Result<crate::Settlement, crate::Error> {
                if let Some(cached) = self.observation.settled() {
                    return cached;
                }
                let budget = super::receipt::settlement_budget(&self.settlement, timeout)?;
                let deadline = self.owner.deadline_after(budget)?;
                self.applied_until(deadline) $($await)* ?;
                match super::receipt::position_poll(
                    &self.settlement,
                    &self.observation,
                    self.affected_axes,
                )? {
                    Some(poll) => {
                        self.observation.check_settlement()?;
                        let result = self
                            .owner
                            .poll_settlement(poll, deadline)
                            $($await)*
                            .map(|()| {
                                crate::Settlement::observed_stable(
                                    poll.axes,
                                    poll.interval,
                                    poll.tolerance,
                                )
                            })
                            .map_err(|error| super::settlement_error(error, self.observation.id()));
                        self.observation.commit_settlement(result, true)
                    }
                    None => self.observation.commit_settlement(
                        Ok(crate::Settlement::profile_completion(self.affected_axes)),
                        false,
                    ),
                }
            }
        }
    };
}

pub(super) use owner_handle_methods;
