# Entry-table construction and removed audit rules (#859)

`EntryTable` owns live entries and all ready, sequence, and socket indexes. Its fields are private; immutable lookups do not expose a mutation path. Admission creates a fresh entry, closed changes update the entry and its index delta in place, and removal forgets every active claim. Transitions retain the existing BTreeMap node, avoiding a new allocation per phase change.

The following 31 former auditor refusals have no independent state left to reconcile. The remaining capacity, reserve, raw-hold, stream-ledger, and raw-candidate checks remain semantic safety audits. No equivalent index audit moved into the table. Socket admission still refuses an occupied resource before changing a lifecycle.

| Former refusal | Why it is enforced by construction |
| --- | --- |
| transmission bound exceeded | Write ownership and the framing expectation are carried by the named entry's exact sending lifecycle. There is no transmission index to reconcile. |
| transmission refers to missing entry | Write ownership and the framing expectation are carried by the named entry's exact sending lifecycle. There is no transmission index to reconcile. |
| transmission generation or attempt is stale | Write ownership and the framing expectation are carried by the named entry's exact sending lifecycle. There is no transmission index to reconcile. |
| transmission is incompatible with authoritative state | Write ownership and the framing expectation are carried by the named entry's exact sending lifecycle. There is no transmission index to reconcile. |
| sending entry has no transmission owner | Write ownership and the framing expectation are carried by the named entry's exact sending lifecycle. There is no transmission index to reconcile. |
| sending entry transmission owner is incompatible | Write ownership and the framing expectation are carried by the named entry's exact sending lifecycle. There is no transmission index to reconcile. |
| cancellation sending entry has no transmission owner | Write ownership and the framing expectation are carried by the named entry's exact sending lifecycle. There is no transmission index to reconcile. |
| cancellation transmission owner is incompatible | Write ownership and the framing expectation are carried by the named entry's exact sending lifecycle. There is no transmission index to reconcile. |
| raw inquiry FIFO bound exceeded | The current inquiry is derived from live inquiry phases and their required transmission send order. No separate FIFO position exists. |
| raw inquiry owns more than one FIFO position | The current inquiry is derived from live inquiry phases and their required transmission send order. No separate FIFO position exists. |
| raw inquiry FIFO refers to missing entry | The current inquiry is derived from live inquiry phases and their required transmission send order. No separate FIFO position exists. |
| raw inquiry FIFO owner is stale | The current inquiry is derived from live inquiry phases and their required transmission send order. No separate FIFO position exists. |
| raw inquiry entry does not own exactly one FIFO position | The current inquiry is derived from live inquiry phases and their required transmission send order. No separate FIFO position exists. |
| retroactive evidence bound exceeded | Deferred evidence is a boundary-local return value. No persistent scratch fields or end-of-input emptiness auditor remain. |
| displaced socket owner bound exceeded | Deferred evidence is a boundary-local return value. No persistent scratch fields or end-of-input emptiness auditor remain. |
| dispatch queue refers to missing entry | Admission, phase transition and removal update private queues eagerly. Queues contain only RequestId, with no generations or stale-ticket path. |
| dispatch queue ticket is stale or misplaced | Admission, phase transition and removal update private queues eagerly. Queues contain only RequestId, with no generations or stale-ticket path. |
| dispatch queue bound exceeded | Admission, phase transition and removal update private queues eagerly. Queues contain only RequestId, with no generations or stale-ticket path. |
| ready queue ticket is stale | Admission, phase transition and removal update private queues eagerly. Queues contain only RequestId, with no generations or stale-ticket path. |
| ready entry does not own exactly one queue position | Admission, phase transition and removal update private queues eagerly. Queues contain only RequestId, with no generations or stale-ticket path. |
| correlation index bound exceeded | Only table admission/transition/removal update history and full/lower indexes together. History starts empty and receipt transitions enforce the fixed maximum. |
| sequence owner refers to missing entry | Only table admission/transition/removal update history and full/lower indexes together. History starts empty and receipt transitions enforce the fixed maximum. |
| sequence owner is stale or phase-incompatible | Only table admission/transition/removal update history and full/lower indexes together. History starts empty and receipt transitions enforce the fixed maximum. |
| lower16 owner refers to missing entry | Only table admission/transition/removal update history and full/lower indexes together. History starts empty and receipt transitions enforce the fixed maximum. |
| lower16 owner is stale | Only table admission/transition/removal update history and full/lower indexes together. History starts empty and receipt transitions enforce the fixed maximum. |
| sequence history bound exceeded | Only table admission/transition/removal update history and full/lower indexes together. History starts empty and receipt transitions enforce the fixed maximum. |
| entry sequence history is absent from correlation index | Only table admission/transition/removal update history and full/lower indexes together. History starts empty and receipt transitions enforce the fixed maximum. |
| socket owner refers to missing entry | A typed two-slot map is updated with the closed active/displaced phase placement. No integer socket key or outside mutation can create a stale claim. |
| socket index is invalid | A typed two-slot map is updated with the closed active/displaced phase placement. No integer socket key or outside mutation can create a stale claim. |
| socket owner is stale or phase-incompatible | A typed two-slot map is updated with the closed active/displaced phase placement. No integer socket key or outside mutation can create a stale claim. |
| socket-owning entry is absent from socket index | A typed two-slot map is updated with the closed active/displaced phase placement. No integer socket key or outside mutation can create a stale claim. |

Ready queues hold request ids, without queue generations or pruning. Sequence records hold only the sequence and response kind; target identity derives from the immutable request. Removing an evicted full sequence preserves a low-16 owner while another retained record of the same kind still uses those bits.

`EntryPlacement::Displaced` privately proves that its one logical lifecycle was socketed. It relinquishes active indexes without dropping the logical phase used by ordered transition effects. Removing it cannot clear a successor’s claim. Ordinary due selection sees active phases only; displacement cleanup completes within the same decision.

Each boundary owns a local `Decision`, which consumes `Answered` retroactive frames by value and returns outward effects only after deferred evidence is processed. There is no engine scratch or end-of-input emptiness audit. A real maximal ledger decision returns seven inline frames; the local all-target batch has 51 inline frames and 14 unique displaced identities.

`EngineTurn` has exactly `Complete` and `InputOnly`. The unreachable public `DiagnosticIgnoreReason::StaleQueueTicket` was removed as part of the breaking 2.0 change; the net public API snapshot must reflect that removal.

## Representation measurements

Measured with Rust 1.98.0 on x86_64 with default `blocking`, immediately before
this change (`8bd6c411`, including #861 timing proofs) and after it:

| Type | Before (bytes) | After (bytes) |
| --- | ---: | ---: |
| `ProtocolEngine` | 3472 | 2968 |
| `Entry` | 536 | 536 |

The engine shrinks by 504 bytes. Entry size stays constant while its sending
phases gain the required framing expectation and inquiry send order; queue
generation and repeated sequence-target facts disappear. Removed per-write
transmission-map nodes and ready-queue ticket width are additional heap savings
not included in these inline sizes. The local decision stores deferred evidence
on the stack, bounded independently of admitted request count.
