//! A processor borrowing an actual raw-stream state and the shared engine.
use super::correlation::{StreamCorrelation, TerminalFamily};
use super::stream_ledger::FollowCandidate;
use super::*;

/// The origin of a socket-naming error other than a cancellation reply.
enum NamedErrorOrigin {
    Execution(RequestId),
    StepFollow {
        candidate: FollowCandidate,
        could_reject: bool,
    },
    HeldSocket {
        could_reject: bool,
    },
    WaitingRejection,
    Ambiguous,
    Unmatched,
}

pub(super) struct StreamContext<'a> {
    pub(super) core: &'a mut EngineCore,
    pub(super) stream: &'a mut StreamCorrelation,
}
impl std::ops::Deref for StreamContext<'_> {
    type Target = EngineCore;
    fn deref(&self) -> &EngineCore {
        self.core
    }
}
impl std::ops::DerefMut for StreamContext<'_> {
    fn deref_mut(&mut self) -> &mut EngineCore {
        self.core
    }
}
impl EngineAccess for StreamContext<'_> {
    fn mode(&self) -> CorrelationRef<'_> {
        CorrelationRef::RawStream(self.stream)
    }
    fn split(&mut self) -> (&mut EngineCore, CorrelationState<'_>) {
        (self.core, CorrelationState::RawStream(self.stream))
    }
}
impl StreamContext<'_> {
    /// Downgrades an exact raw socket tombstone superseded by a camera ACK.
    ///
    /// An inert `Socket` hold records only our uncertainty about a predecessor
    /// whose terminal frame was lost. A later ACK naming that same socket is
    /// authoritative evidence that the camera released it and assigned it to
    /// the uniquely resolved live request. Retain the predecessor's ambiguity
    /// window as an unkeyed `PreAck` hold, but never let the stale exact key
    /// reject the camera-named successor (#750). A byte stream needs no
    /// `PreAck` hold: the ledger owes every unanswered first answer (#795).
    pub(super) fn quarantine_displaced_socket_hold(
        &mut self,
        target: TargetSlot,
        socket: ViscaSocket,
    ) {
        if self.stream.holds.release_socket(target, socket).is_some() {
            self.stream.learning[target].pending_socketless_completions = None;
        }
    }
    pub(super) fn resolve_release_gate(
        &mut self,
        now: Instant,
        evidence: Option<RawPrefixEvidence>,
    ) -> RawReleaseGateAction {
        let releases = self.raw_correlation_releases_due(now);
        let Some(evidence) = evidence else {
            self.stream.release_gate = None;
            return RawReleaseGateAction::Advance;
        };
        if releases.is_empty() {
            self.stream.release_gate = None;
            return RawReleaseGateAction::Advance;
        }
        match self.raw_prefix_disposition(releases, evidence) {
            RawPrefixDisposition::NoRelease | RawPrefixDisposition::ReleasePreserving => {
                self.stream.release_gate = None;
                RawReleaseGateAction::Advance
            }
            RawPrefixDisposition::Discard => {
                self.stream.release_gate = None;
                RawReleaseGateAction::DiscardFirst
            }
            RawPrefixDisposition::Defer => {
                let await_until = self
                    .stream
                    .release_gate
                    .filter(|gate| gate.releases == releases)
                    .map_or_else(
                        || add_duration(now, self.policy.raw_release_grace),
                        |gate| gate.await_until,
                    );
                if now >= await_until {
                    self.stream.release_gate = None;
                    RawReleaseGateAction::DiscardFirst
                } else {
                    self.stream.release_gate = Some(RawReleaseGate {
                        releases,
                        await_until,
                    });
                    RawReleaseGateAction::AwaitInputUntil(await_until)
                }
            }
        }
    }

    /// Turns the live ledger entries of a request that ends without its first
    /// answer into debts, or drops them if their write never left.
    ///
    /// A debt's window is the request's ambiguity interval (an inquiry's is at
    /// least the reply skew), started now. An owed raw stream inquiry reply
    /// also retains the declaration's inquiry hold through that window, so
    /// overlapping inquiries wait; that class then latches instead of expiring.
    pub(super) fn retire_stream_answers(
        &mut self,
        id: RequestId,
        now: Instant,
        effects: &mut Decision,
    ) {
        let Some((target, timeout, shape)) = self.entries.get(&id).map(|entry| {
            (
                entry.request.context().target,
                entry.request.context().timeout,
                entry.request.payload_shape(),
            )
        }) else {
            return;
        };
        if self.stream.ledger.live(target, id).next().is_none() {
            return;
        }
        let mut reply_window_end = add_duration(
            now,
            timeout
                .ambiguity
                .get()
                .max(self.policy.raw_inquiry_release_hold),
        );
        let reply_written = self
            .stream
            .ledger
            .live(target, id)
            .any(|entry| entry.written && matches!(entry.owes, Owes::Reply(_)));
        if let (Some(shape), true) = (shape, reply_written) {
            self.stream.holds.extend(
                target,
                StreamScope::Inquiry(shape),
                reply_window_end,
                Some(id),
            );
            if let Some(hold) = self.raw_hold(target, RawHoldScope::Inquiry(shape)) {
                reply_window_end = hold.until;
            }
        }
        let command_window_end = add_duration(now, timeout.ambiguity.get());
        let answered = self.stream.ledger.retire(target, id, |owes| match owes {
            Owes::Reply(_) => (reply_window_end, Duration::ZERO),
            Owes::Ack
            | Owes::Completion
            | Owes::AcceptedCompletion
            | Owes::Rejection
            | Owes::Cancel(_) => (command_window_end, timeout.completion.get()),
        });
        self.absorb_answered(target, answered, now, effects);
    }

    /// Retires a strictly expired candidate before consuming its answer.
    /// Exact-deadline input still wins. Only candidates for this frame advance;
    /// an unrelated expired request cannot suppress input for a live request.
    pub(super) fn retire_stale_stream_candidates(
        &mut self,
        frame: &DecodedFrame,
        now: Instant,
        effects: &mut Decision,
    ) {
        let target = frame.target;
        let can_end_unassigned = match frame.response {
            DecodedResponse::Completion { .. } => true,
            DecodedResponse::Error {
                socket: Some(_),
                code,
            } => !code.answers_named_cancellation(),
            _ => false,
        };
        if can_end_unassigned {
            while let Some(id) = self.entries.iter().find_map(|(id, entry)| {
                (entry.request.context().target == target
                    && matches!(
                        entry.lifecycle(),
                        Lifecycle::ExecutingUnassigned { deadline, .. } if deadline < now
                    ))
                .then_some(*id)
            }) {
                self.retire_stale_correlated_request(id, now, effects);
            }
        }
        let (answer, socket) = match &frame.response {
            DecodedResponse::Ack { .. } => (Some(Answer::Ack), None),
            DecodedResponse::InquiryReply { payload } => (Some(Answer::Reply(payload)), None),
            DecodedResponse::Completion { socket } => {
                (socket.is_none().then_some(Answer::Completion), *socket)
            }
            DecodedResponse::Error {
                socket: Some(socket),
                code,
            } if code.answers_named_cancellation() => {
                (Some(Answer::CancelReply(*socket)), Some(*socket))
            }
            DecodedResponse::Error { socket, .. } => (
                Some(if socket.is_some() {
                    Answer::NamedRejection
                } else {
                    Answer::SocketlessError
                }),
                *socket,
            ),
            DecodedResponse::SonyControl { .. }
            | DecodedResponse::NetworkChange
            | DecodedResponse::Unknown => (None, None),
        };
        let candidates = answer.map_or([None, None], |answer| {
            self.stream.ledger.candidates(target, answer)
        });
        let mut requests = SmallVec::<[RequestId; 3]>::new();
        if let Some(owner) = socket.and_then(|socket| self.socket_owner(target, socket)) {
            requests.push(owner);
        }
        if matches!(frame.response, DecodedResponse::Completion { socket: None })
            && !self.stream.learning[target].completion_sockets.is_named()
            && !self.stream.ledger.owes_completion(target)
            && self.raw_socket_hold_count(target) == 0
            && self.raw_hold(target, RawHoldScope::AllResponses).is_none()
        {
            if let Some(owner) = self.sole_socket_holder(target) {
                if !requests.contains(&owner) {
                    requests.push(owner);
                }
            }
        }
        for candidate in candidates
            .into_iter()
            .flatten()
            .filter(|candidate| !candidate.is_debt())
        {
            if !requests.contains(&candidate.request) {
                requests.push(candidate.request);
            }
        }
        for id in requests {
            let stale = self.entries.get(&id).is_some_and(|entry| {
                entry
                    .correlated_response_deadline()
                    .is_some_and(|deadline| deadline < now)
            });
            if !stale {
                continue;
            }
            self.retire_stale_correlated_request(id, now, effects);
        }
    }

    fn stream_owes_ack(&self, target: TargetSlot) -> bool {
        self.stream.ledger.candidates(target, Answer::Ack)[0].is_some()
    }

    fn classify_named_error(
        &self,
        target: TargetSlot,
        socket: ViscaSocket,
        code: CameraErrorCode,
    ) -> NamedErrorOrigin {
        let could_reject = code.refuses_before_start()
            && self
                .stream
                .ledger
                .candidates(target, Answer::NamedRejection)[0]
                .is_some();
        let owner = self.socket_owner(target, socket);
        let follow = self.stream.ledger.follow_candidate(target, socket);
        let held = self
            .raw_hold(target, RawHoldScope::Socket(socket))
            .is_some();
        // A rejection names the free socket the camera allocated for the
        // rejected command. Without named completions, a socket we still
        // associate with an execution may have become free unseen.
        if could_reject
            && (owner.is_some() || follow.is_some() || held)
            && !self.stream.learning[target].completion_sockets.is_named()
        {
            NamedErrorOrigin::Ambiguous
        } else if let Some(owner) = owner {
            NamedErrorOrigin::Execution(owner)
        } else if let Some(follow) = follow {
            NamedErrorOrigin::StepFollow {
                candidate: follow,
                could_reject,
            }
        } else if held {
            NamedErrorOrigin::HeldSocket { could_reject }
        } else if could_reject {
            NamedErrorOrigin::WaitingRejection
        } else {
            NamedErrorOrigin::Unmatched
        }
    }

    fn socket_capacity(&self, target: TargetSlot) -> CommandSocketCount {
        let physical = self.targets[target]
            .policy
            .map_or(CommandSocketCount::TWO, |policy| {
                policy.physical_command_sockets
            });
        self.stream.learning[target]
            .socket_capacity_seen
            .map_or(physical, |seen| physical.max(seen))
    }

    /// Bounds alone never finish live requests. Once no terminal is owed and
    /// no live socket owner remains, old correlation holds cannot intercept
    /// a successor's answers.
    pub(super) fn release_idle_sockets(&mut self, target: TargetSlot) {
        if !self.stream.learning[target].terminals.proved_idle
            || [ViscaSocket::S1, ViscaSocket::S2]
                .into_iter()
                .any(|socket| self.socket_owner(target, socket).is_some())
            || self.raw_live_unassigned(target)
        {
            return;
        }
        for socket in [ViscaSocket::S1, ViscaSocket::S2] {
            self.stream.holds.release_socket(target, socket);
        }
        self.stream.holds.release_unassigned_execution(target);
        let learning = &mut self.stream.learning[target];
        learning.terminals.proved_idle = false;
        learning.pending_socketless_completions = None;
    }

    /// A conclusive first answer for the sole possible rejection candidate
    /// proves that a previous ambiguous named error was an execution terminal.
    fn prove_first_answer(&mut self, target: TargetSlot, answer: Answer<'_>) {
        let terminals = &mut self.stream.learning[target].terminals;
        if !terminals.awaits_named_proof() {
            return;
        }
        if let Some(entry) = self
            .stream
            .ledger
            .sole_written_candidate(target, answer)
            .filter(|entry| entry.owes == Owes::Ack)
        {
            if terminals.prove_first_answer(entry.order) {
                self.release_idle_sockets(target);
            }
        }
    }

    fn observe_named_error(&mut self, target: TargetSlot, socket: ViscaSocket, could_reject: bool) {
        let terminals = &mut self.stream.learning[target].terminals;
        if could_reject {
            let waiting = self
                .stream
                .ledger
                .sole_written_candidate(target, Answer::NamedRejection)
                .map(|entry| entry.order);
            terminals.disputed_named_error(socket, waiting);
        } else {
            terminals.terminal(TerminalFamily::Socketed);
        }
    }

    /// Resolves an unsequenced frame on a raw byte stream through the ledger,
    /// returning the live request it answers.
    ///
    /// Each first-answer frame resolves against the oldest outstanding entry
    /// that can legally produce it ([`stream_ledger::Answer`]); when that
    /// entry is a debt the frame is discarded. A completion, or an error naming
    /// an executing command's socket, answers that command rather than any
    /// first answer.
    pub(super) fn resolve_raw_stream(
        &mut self,
        frame: &DecodedFrame,
        now: Instant,
        effects: &mut Decision,
    ) -> Option<RequestId> {
        let target = frame.target;
        if matches!(
            frame.response,
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S2)
            } | DecodedResponse::Completion {
                socket: Some(ViscaSocket::S2)
            } | DecodedResponse::Error {
                socket: Some(ViscaSocket::S2),
                ..
            }
        ) {
            self.stream.learning[target].socket_capacity_seen = Some(CommandSocketCount::TWO);
        }
        let evidence = match frame.response {
            DecodedResponse::Ack { socket } => Evidence::Ack(socket),
            DecodedResponse::Error { code, .. } => Evidence::Error(code),
            _ => Evidence::Other,
        };
        match &frame.response {
            DecodedResponse::Ack { socket } => {
                self.prove_first_answer(target, Answer::Ack);
                let answered = self.stream.ledger.answer(target, Answer::Ack, evidence);
                let resolved = self.absorb_answered(target, answered, now, effects);
                let capacity = self.socket_capacity(target);
                self.stream.learning[target].terminals.observe_ack(capacity);
                match resolved {
                    Resolved::Live(entry) => Some(entry.request),
                    Resolved::Debt(debt) => {
                        self.displace_stale_socket_owner(target, *socket, effects);
                        if let Standing::Owed { completion, .. } = debt.standing {
                            self.hold_executing_socket(
                                target,
                                *socket,
                                completion,
                                Some(debt.request),
                                now,
                            );
                        }
                        None
                    }
                    // Bound to neither candidate, the ACK still shows that
                    // one of them executes in the socket it names: hold it,
                    // so that its completion or error is never taken for
                    // another command's (or released as a held one's).
                    Resolved::Disputed(candidates) => {
                        self.displace_stale_socket_owner(target, *socket, effects);
                        self.hold_disputed_command_socket(target, *socket, &candidates, now);
                        None
                    }
                    Resolved::Nothing => None,
                }
            }
            DecodedResponse::InquiryReply { payload } => self
                .take_stream_answer(target, Answer::Reply(payload), evidence, now, effects)?
                .ok(),
            // A completion naming a socket is the completion of a command
            // that earned that socket: its live owner's, or the late one of a
            // quarantined or owed command (whose hold it releases). It never
            // completes a `CompletionOnly` command, which earns no socket: a
            // command's completion can outlast its hold.
            DecodedResponse::Completion {
                socket: Some(socket),
            } => {
                self.stream.update_learning(target, |learning| {
                    learning.terminals.terminal(TerminalFamily::Socketed);
                    learning.completion_sockets.learn_named();
                });
                if self.raw_execution_unassigned(target) {
                    let held = self
                        .raw_hold(target, RawHoldScope::Socket(*socket))
                        .is_some();
                    let follow = self
                        .stream
                        .ledger
                        .follow_candidate(target, *socket)
                        .is_some();
                    if !held && !follow && !self.stream_owes_ack(target) {
                        let owner = self.sole_raw_execution_candidate(target, Some(*socket))?;
                        self.settle_before_owner(target, owner, now, effects);
                        return Some(owner);
                    }
                    return None;
                }
                if let Some(owner) = self.socket_owner(target, *socket) {
                    self.settle_before_owner(target, owner, now, effects);
                    return Some(owner);
                }
                if let Some(candidate) = self.stream.ledger.follow_candidate(target, *socket) {
                    self.stream.ledger.record_follow(
                        target,
                        candidate,
                        NamedFrame::Completion(*socket),
                    );
                }
                self.stream.holds.release_socket(target, *socket);
                None
            }
            // A compatible ACK left execution unkeyed. Before using the
            // normal socket or completion-only ledger, check every source that
            // could have emitted this terminal.
            DecodedResponse::Completion { socket: None }
                if self.raw_execution_unassigned(target) =>
            {
                self.learn_socketless_completion(target);
                self.stream.learning[target]
                    .terminals
                    .terminal(TerminalFamily::Either);
                self.stream.learning[target].pending_socketless_completions = None;
                if !self.stream.ledger.owes_completion(target)
                    && !self.stream_owes_ack(target)
                    && self.raw_socket_hold_count(target) == 0
                {
                    let owner = self.sole_raw_execution_candidate(target, None)?;
                    self.settle_before_owner(target, owner, now, effects);
                    return Some(owner);
                }
                None
            }
            // A camera whose completions name their sockets completes a
            // socketed command with its socket: a socketless completion is a
            // `CompletionOnly` command's, whatever executes or is held. One
            // that arrives with no `CompletionOnly` command outstanding proves
            // the camera omits the nibble, for the rest of the session.
            DecodedResponse::Completion { socket: None }
                if self.learn_socketless_completion(target).is_named() =>
            {
                self.stream.learning[target]
                    .terminals
                    .terminal(TerminalFamily::CompletionOnly);
                self.take_stream_answer(target, Answer::Completion, evidence, now, effects)?
                    .ok()
            }
            DecodedResponse::Completion { socket: None } => {
                self.stream.learning[target]
                    .terminals
                    .terminal(TerminalFamily::Either);
                let owners = [ViscaSocket::S1, ViscaSocket::S2]
                    .map(|socket| self.socket_owner(target, socket));
                let holders = owners.iter().flatten().count();
                let held: SmallVec<[ViscaSocket; 2]> = [ViscaSocket::S1, ViscaSocket::S2]
                    .into_iter()
                    .filter(|socket| {
                        self.raw_hold(target, RawHoldScope::Socket(*socket))
                            .is_some()
                    })
                    .collect();
                if self.stream.ledger.owes_completion(target) {
                    // A `CompletionOnly` command's, unless an executing or
                    // quarantined command could have sent it.
                    if holders != 0 || !held.is_empty() {
                        self.stream.update_learning(target, |learning| {
                            learning.completion_sockets.drop_ambiguous();
                        });
                        return None;
                    }
                    return self
                        .take_stream_answer(target, Answer::Completion, evidence, now, effects)?
                        .ok();
                }
                // A stream-wide hold is for a NoReply command, which earns
                // no completion. Any socket holder's completion still counts.
                match (holders, held.as_slice()) {
                    (0, [socket]) => {
                        self.stream.update_learning(target, |learning| {
                            learning.pending_socketless_completions = None
                        });
                        self.stream.holds.release_socket(target, *socket);
                        None
                    }
                    (_, []) => {
                        if holders > 1 {
                            self.retain_ambiguous_socketless_completion(target, now, effects);
                            return None;
                        }
                        self.stream.update_learning(target, |learning| {
                            learning.pending_socketless_completions = None
                        });
                        self.sole_socket_holder(target)
                    }
                    _ => {
                        self.retain_ambiguous_socketless_completion(target, now, effects);
                        None
                    }
                }
            }
            DecodedResponse::Error {
                socket: Some(socket),
                code,
            } => {
                if code.answers_named_cancellation() {
                    if code.confirms_cancellation() {
                        self.stream.learning[target]
                            .terminals
                            .terminal(TerminalFamily::Socketed);
                    }
                    return match self.take_stream_answer(
                        target,
                        Answer::CancelReply(*socket),
                        evidence,
                        now,
                        effects,
                    )? {
                        Ok(id) => Some(id),
                        // The late answer of an ended command's cancellation:
                        // that command no longer runs there.
                        Err(debt) => {
                            self.stream.holds.release_socket(target, *socket);
                            self.end_command_a_late_cancel_reached(
                                target, *socket, *code, debt.order, effects,
                            );
                            None
                        }
                    };
                }
                if self.raw_execution_unassigned(target) {
                    let could_reject = code.refuses_before_start()
                        && self
                            .stream
                            .ledger
                            .candidates(target, Answer::NamedRejection)[0]
                            .is_some();
                    self.observe_named_error(target, *socket, could_reject);
                    if could_reject {
                        self.stream.ledger.dispute(target, Answer::NamedRejection);
                        return None;
                    }
                    if self
                        .raw_hold(target, RawHoldScope::Socket(*socket))
                        .is_some()
                        || self
                            .stream
                            .ledger
                            .follow_candidate(target, *socket)
                            .is_some()
                    {
                        return None;
                    }
                    let owner = self.sole_raw_execution_candidate(target, Some(*socket))?;
                    self.settle_before_owner(target, owner, now, effects);
                    return Some(owner);
                }
                match self.classify_named_error(target, *socket, *code) {
                    NamedErrorOrigin::Execution(owner) => {
                        self.observe_named_error(target, *socket, false);
                        self.settle_before_owner(target, owner, now, effects);
                        Some(owner)
                    }
                    NamedErrorOrigin::StepFollow {
                        candidate,
                        could_reject,
                    } => {
                        self.observe_named_error(target, *socket, could_reject);
                        self.stream.ledger.record_follow(
                            target,
                            candidate,
                            NamedFrame::Error(*socket, *code),
                        );
                        self.stream.holds.release_socket(target, *socket);
                        None
                    }
                    NamedErrorOrigin::HeldSocket { could_reject } => {
                        self.observe_named_error(target, *socket, could_reject);
                        self.stream.holds.release_socket(target, *socket);
                        None
                    }
                    NamedErrorOrigin::WaitingRejection => self
                        .take_stream_answer(target, Answer::NamedRejection, evidence, now, effects)?
                        .ok(),
                    NamedErrorOrigin::Ambiguous => {
                        self.observe_named_error(target, *socket, true);
                        self.stream.ledger.dispute(target, Answer::NamedRejection);
                        self.stream.holds.release_socket(target, *socket);
                        None
                    }
                    NamedErrorOrigin::Unmatched => {
                        // With no legal first-answer candidate, this can only
                        // be an execution terminal.
                        self.observe_named_error(target, *socket, false);
                        None
                    }
                }
            }
            // A socketless error is a rejection: an execution error names its
            // socket. It is therefore the first answer of the oldest
            // outstanding request.
            DecodedResponse::Error { socket: None, .. } => self
                .take_stream_answer(target, Answer::SocketlessError, evidence, now, effects)?
                .ok(),
            DecodedResponse::SonyControl { .. }
            | DecodedResponse::NetworkChange
            | DecodedResponse::Unknown => None,
        }
    }

    /// A completion cannot choose between a held and a live socket. Keep its
    /// candidate identities, and account it only when a terminal count proves
    /// that every one ended. An ACK reusing a held socket alone is insufficient:
    /// the retained frame might have been that predecessor's completion.
    pub(super) fn retain_ambiguous_socketless_completion(
        &mut self,
        target: TargetSlot,
        now: Instant,
        effects: &mut Decision,
    ) {
        let mut candidates = [None; 2];
        for socket in [ViscaSocket::S1, ViscaSocket::S2] {
            candidates[socket.as_index()] = self.socket_owner(target, socket);
            if let Some(hold) = self.raw_hold(target, RawHoldScope::Socket(socket)) {
                let Some(owner) = hold.owner else {
                    self.stream.update_learning(target, |learning| {
                        learning.pending_socketless_completions = None
                    });
                    return;
                };
                candidates[socket.as_index()] = Some(owner);
            }
        }
        let mut pending = self
            .stream
            .take_pending(target)
            .filter(|pending| pending.candidates == candidates)
            .unwrap_or(PendingSocketlessCompletions {
                candidates,
                count: 0,
            });
        pending.count = pending.count.saturating_add(1);
        if usize::from(pending.count) < candidates.iter().flatten().count() {
            self.stream.update_learning(target, |learning| {
                learning.pending_socketless_completions = Some(pending)
            });
            return;
        }
        for id in candidates.into_iter().flatten() {
            self.retire_stale_correlated_request(id, now, effects);
        }
        for socket in [ViscaSocket::S1, ViscaSocket::S2] {
            self.stream.holds.release_socket(target, socket);
            if let Some(id) = self.socket_owner(target, socket) {
                self.settle_before_owner(target, id, now, effects);
                if self.entries.get(&id).is_some() {
                    effects.defer_frame(Retro {
                        request: id,
                        evidence: Evidence::Named(NamedFrame::Completion(socket)),
                    });
                }
            }
        }
    }

    /// Once a camera is known to name its sockets in completions, the
    /// socketless completions dropped as ambiguous before were `CompletionOnly`
    /// commands': each completes the oldest such command still owed (or pays
    /// its debt), and any left over are forgotten. Once the camera is known
    /// to omit them, they stay dropped.
    pub(super) fn pay_learned_completions(&mut self, now: Instant, effects: &mut Decision) {
        for target in TargetSlot::ALL {
            self.pay_learned_completions_for_target(target, now, effects);
        }
    }

    /// Pay newly attributed evidence after its target's named frame, before
    /// the next frame can emit an observer or diagnostic event.
    pub(super) fn pay_learned_completions_for_target(
        &mut self,
        target: TargetSlot,
        now: Instant,
        effects: &mut Decision,
    ) {
        while let Some((entry, answered)) = self.stream.next_learned_completion(target) {
            self.absorb_answered(target, answered, now, effects);
            let awaiting = self
                .entries
                .get(&entry.request)
                .is_some_and(|live| live.lifecycle().is_awaiting_completion());
            if awaiting {
                self.completion(entry.request, None, now, effects);
            }
        }
    }

    /// Applies a first-answer frame to the ledger ([`StreamLedger::answer`]):
    /// `Ok` with the live request it binds to, `Err` with the debt it paid,
    /// or `None` when it binds to nothing (no candidate, or the ledger's one
    /// disputed answer). Frames a proven dispute assigns to their requests
    /// are queued for [`Self::apply_retro`]. A paid or retired inquiry reply
    /// keeps only the ordinary reply skew from now before that lane reopens.
    pub(super) fn take_stream_answer(
        &mut self,
        target: TargetSlot,
        answer: Answer<'_>,
        evidence: Evidence,
        now: Instant,
        effects: &mut Decision,
    ) -> Option<Result<RequestId, Outstanding>> {
        if matches!(answer, Answer::NamedRejection | Answer::SocketlessError) {
            self.prove_first_answer(target, answer);
        }
        let answered = self.stream.ledger.answer(target, answer, evidence);
        if answer == Answer::SocketlessError {
            let terminals = &mut self.stream.learning[target].terminals;
            match answered.resolved {
                Resolved::Live(entry) | Resolved::Debt(entry)
                    if entry.owes == Owes::Completion && entry.written =>
                {
                    terminals.terminal(TerminalFamily::CompletionOnly);
                }
                Resolved::Disputed(_) => {
                    terminals.disputed_socketless_error(answered.completion_dispute);
                }
                _ => {}
            }
        }
        match self.absorb_answered(target, answered, now, effects) {
            Resolved::Live(entry) => Some(Ok(entry.request)),
            Resolved::Debt(entry) => Some(Err(entry)),
            Resolved::Nothing | Resolved::Disputed(_) => None,
        }
    }

    /// A cancellation written for an ended command after another command was
    /// written, which the camera then put in the same socket, reached that
    /// command instead: the camera reads in write order, so by the time it
    /// read the cancellation the socket was the later command's. Its `0x04`
    /// therefore cancelled the command now owning the socket, which ends
    /// unconfirmed (it may have partly run), and the socket is free; a later
    /// rejection naming it is then never taken for that command's error.
    pub(super) fn end_command_a_late_cancel_reached(
        &mut self,
        target: TargetSlot,
        socket: ViscaSocket,
        code: CameraErrorCode,
        cancel_order: u64,
        effects: &mut Decision,
    ) {
        if !code.confirms_cancellation() {
            return;
        }
        let Some(owner) = self.socket_owner(target, socket) else {
            return;
        };
        let reached = self
            .entries
            .get(&owner)
            .and_then(|entry| entry.stream_order())
            .is_some_and(|order| order < cancel_order);
        if reached {
            effects.displace(owner);
        }
    }

    /// Applies what the ledger settled: retroactive dispute assignments, a
    /// `CompletionOnly` entry a dispute retired, and an owed inquiry reply
    /// paid or retired (which keeps only the ordinary reply skew from now).
    pub(super) fn absorb_answered(
        &mut self,
        target: TargetSlot,
        answered: Answered,
        now: Instant,
        effects: &mut Decision,
    ) -> Resolved {
        if answered.completion_only_retired {
            self.stream.update_learning(target, |learning| {
                learning.terminals.settle_completion_rejection();
                learning.completion_sockets.retire_completion();
            });
        }
        effects.defer_frames(answered.retro);
        let skew_until = add_duration(now, self.policy.raw_inquiry_release_hold);
        for shape in answered.replies_settled.iter() {
            self.stream
                .holds
                .reset_paid_reply_skew(target, shape, skew_until);
        }
        answered.resolved
    }

    /// A completion or execution error naming the socket of `owner`, which
    /// executes there, follows `owner`'s first answer and so every first
    /// answer written before it (#795): whatever the stream still owes for
    /// those is settled, whatever time it is.
    pub(super) fn settle_before_owner(
        &mut self,
        target: TargetSlot,
        owner: RequestId,
        now: Instant,
        effects: &mut Decision,
    ) {
        let Some(order) = self
            .entries
            .get(&owner)
            .and_then(|entry| entry.stream_order())
        else {
            return;
        };
        let answered = self.stream.ledger.settle_before(target, order);
        self.absorb_answered(target, answered, now, effects);
    }

    /// Holds the socket a disputed ACK names, since one of its two
    /// `candidates` executes there, for the longer of their completion
    /// windows. The write-first input contract ensures each candidate's write
    /// result is already known when this ACK is processed. Like an owed
    /// command's hold, evidence
    /// releases it: a completion or error naming the socket, or another ACK
    /// naming it.
    pub(super) fn hold_disputed_command_socket(
        &mut self,
        target: TargetSlot,
        socket: Option<ViscaSocket>,
        candidates: &[Outstanding; 2],
        now: Instant,
    ) {
        let completion = candidates
            .iter()
            .filter_map(|candidate| match candidate.standing {
                Standing::Owed { completion, .. } => Some(completion),
                Standing::Live => self
                    .entries
                    .get(&candidate.request)
                    .map(|entry| entry.request.context().timeout.completion.get()),
            })
            .max();
        if let Some(completion) = completion {
            self.hold_executing_socket(target, socket, completion, None, now);
        }
    }

    /// An ACK that binds to no live request names `socket`: the camera put
    /// another command there, so a command the engine still indexes there
    /// lost its completion and has ended (#721). It is released now and
    /// ended with the frame's other effects.
    fn displace_stale_socket_owner(
        &mut self,
        target: TargetSlot,
        socket: Option<ViscaSocket>,
        effects: &mut Decision,
    ) {
        if let Some(stale) = socket.and_then(|socket| self.socket_owner(target, socket)) {
            self.release_attempt_ownership(stale);
            effects.displace(stale);
        }
    }

    /// A compatibility ACK proves execution but supplies no physical key.
    fn hold_executing_socket(
        &mut self,
        target: TargetSlot,
        requested: Option<ViscaSocket>,
        completion: Duration,
        owner: Option<RequestId>,
        now: Instant,
    ) {
        let scope = match requested {
            Some(socket) if self.socket_owner(target, socket).is_none() => {
                StreamScope::Socket(socket)
            }
            Some(_) => return,
            None => StreamScope::UnassignedExecution,
        };
        self.stream
            .holds
            .extend(target, scope, add_duration(now, completion), owner);
    }

    /// Records what a socketless completion shows about `target`'s camera,
    /// and returns what is known: with no `CompletionOnly` command
    /// outstanding, it was a socketed command's, so the camera omits the
    /// socket nibble in completions for the rest of the session.
    pub(super) fn learn_socketless_completion(&mut self, target: TargetSlot) -> CompletionSockets {
        self.stream.learn_socketless_completion(target)
    }
}

/// A raw-datagram processor has a hold table with the pre-ACK scope.
pub(super) struct DatagramContext<'a> {
    pub(super) core: &'a mut EngineCore,
    pub(super) holds: &'a mut holds::HoldTable<RawHoldScope>,
}
impl std::ops::Deref for DatagramContext<'_> {
    type Target = EngineCore;
    fn deref(&self) -> &EngineCore {
        self.core
    }
}
impl std::ops::DerefMut for DatagramContext<'_> {
    fn deref_mut(&mut self) -> &mut EngineCore {
        self.core
    }
}
impl EngineAccess for DatagramContext<'_> {
    fn mode(&self) -> CorrelationRef<'_> {
        CorrelationRef::RawDatagram(self.holds)
    }
    fn split(&mut self) -> (&mut EngineCore, CorrelationState<'_>) {
        (self.core, CorrelationState::RawDatagram(self.holds))
    }
}

impl DatagramContext<'_> {
    pub(super) fn quarantine_displaced_socket_hold(
        &mut self,
        target: TargetSlot,
        socket: ViscaSocket,
    ) {
        if let Some(hold) = self.holds.release_socket(target, socket) {
            self.holds
                .extend(target, RawHoldScope::PreAck, hold.until, hold.owner);
        }
    }
}
