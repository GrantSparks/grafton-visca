//! A STOP keeps the ACK-bearing datagram quarantine even while being written.

use super::*;
use crate::runtime::engine::{correlation::quarantine_datagram, holds::HoldTable};

#[test]
fn sending_stops_keep_the_ack_command_preack_quarantine() {
    let now = Instant::now();
    let until = now + Duration::from_millis(20);
    let target = camera(1);
    let request = RequestId::from_nonzero(NonZeroU64::new(7).unwrap());
    let transmission = TransmissionId::from_nonzero(NonZeroU64::new(9).unwrap());

    for (reply, expected_scope) in [
        (SendingReply::Stop, Some(RawHoldScope::PreAck)),
        (SendingReply::AckThenCompletion, Some(RawHoldScope::PreAck)),
        (
            SendingReply::CompletionOnly,
            Some(RawHoldScope::AllResponses),
        ),
        (SendingReply::NoReply, Some(RawHoldScope::AllResponses)),
        (SendingReply::Inquiry, None),
    ] {
        let lifecycle = Lifecycle::Sending {
            transmission,
            started_at: now,
            attempt: AttemptFacts {
                socket_capacity: Some(CommandSocketCount::TWO),
                stream_order: None,
            },
            reply,
        };
        let mut holds = HoldTable::<RawHoldScope>::new();
        quarantine_datagram(&mut holds, target, request, lifecycle, until);

        if let Some(scope) = expected_scope {
            assert_eq!(
                holds.get(target, scope),
                Some(RawHold {
                    until,
                    owner: Some(request),
                }),
                "{reply:?} retains its reply quarantine and owner"
            );
            assert_eq!(holds.entries().len(), 1, "{reply:?}");
            assert!(holds.get(camera(2), scope).is_none());
            holds.expire(until - Duration::from_nanos(1));
            assert!(!holds.is_empty(), "{reply:?} cannot expire early");
            holds.expire(until);
        }
        assert!(holds.is_empty(), "{reply:?} has no remaining quarantine");
    }
}
