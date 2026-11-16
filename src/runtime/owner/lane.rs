//! Statically paired prepared requests and terminal observation lanes.

use std::{marker::PhantomData, ops::Deref, sync::Arc};

use crate::{
    runtime::engine::{
        AppliedStateProjection, EncodedMessage, ReplyShape, RequestContext, RuntimeRequest,
    },
    PayloadShape,
};

use super::{CommandOutcome, InquiryOutcome, TerminalLane};

/// A prepared request whose terminal lane cannot be selected independently.
/// The constructors take each variant's fields, so a command cannot be wrapped
/// with an inquiry's observer type (or vice versa).
#[derive(Debug, Clone)]
pub(crate) struct LaneRequest<T: TerminalLane> {
    request: RuntimeRequest,
    lane: PhantomData<fn() -> T>,
}

impl LaneRequest<CommandOutcome> {
    pub(crate) fn command(
        reply_shape: ReplyShape,
        wire: Arc<EncodedMessage>,
        context: RequestContext,
        applied_state: Option<AppliedStateProjection>,
    ) -> Self {
        Self {
            request: RuntimeRequest::Command {
                reply_shape,
                wire,
                context,
                applied_state,
            },
            lane: PhantomData,
        }
    }
}

impl LaneRequest<InquiryOutcome> {
    pub(crate) fn inquiry(
        wire: Arc<EncodedMessage>,
        context: RequestContext,
        payload_shape: PayloadShape,
    ) -> Self {
        Self {
            request: RuntimeRequest::Inquiry {
                wire,
                context,
                payload_shape,
            },
            lane: PhantomData,
        }
    }
}

impl<T: TerminalLane> LaneRequest<T> {
    pub(crate) fn into_inner(self) -> RuntimeRequest {
        self.request
    }

    pub(crate) fn context_mut(&mut self) -> &mut RequestContext {
        self.request.context_mut()
    }
}

impl<T: TerminalLane> Deref for LaneRequest<T> {
    type Target = RuntimeRequest;

    fn deref(&self) -> &Self::Target {
        &self.request
    }
}

/// The heterogeneous scripted test drivers deliberately select their own lane.
/// Production has only the two variant-specific prepared constructors above.
#[cfg(test)]
impl<T: TerminalLane> From<RuntimeRequest> for LaneRequest<T> {
    fn from(request: RuntimeRequest) -> Self {
        Self {
            request,
            lane: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{ObserverResolution, TerminalObserver, TerminalSlot};
    use super::*;
    use std::time::Instant;

    #[test]
    fn terminal_slots_deliver_each_matching_lane_and_fail_closed_on_mismatch() {
        let at = Instant::now();
        let (command, cell) = TerminalObserver::<CommandOutcome>::pair();
        assert_eq!(
            TerminalSlot::from(cell).resolve_at(
                crate::runtime::engine::RuntimeOutcome::Command(CommandOutcome::Applied),
                at
            ),
            ObserverResolution::Delivered
        );
        assert!(matches!(command.try_recv(), Some(CommandOutcome::Applied)));
        let (inquiry, cell) = TerminalObserver::<InquiryOutcome>::pair();
        assert_eq!(
            TerminalSlot::from(cell).resolve_at(
                crate::runtime::engine::RuntimeOutcome::Inquiry(InquiryOutcome::Reply {
                    payload: smallvec::smallvec![2]
                }),
                at
            ),
            ObserverResolution::Delivered
        );
        assert!(
            matches!(inquiry.try_recv(), Some(InquiryOutcome::Reply { payload }) if payload.as_slice() == [2])
        );
        let (command, cell) = TerminalObserver::<CommandOutcome>::pair();
        TerminalSlot::from(cell).resolve_at(
            crate::runtime::engine::RuntimeOutcome::Inquiry(InquiryOutcome::Failed(
                crate::Error::SyntaxError,
            )),
            at,
        );
        assert!(matches!(
            command.try_recv(),
            Some(CommandOutcome::Failed(crate::Error::InvalidState { .. }))
        ));
        let (inquiry, cell) = TerminalObserver::<InquiryOutcome>::pair();
        TerminalSlot::from(cell).resolve_at(
            crate::runtime::engine::RuntimeOutcome::Command(CommandOutcome::Applied),
            at,
        );
        assert!(matches!(
            inquiry.try_recv(),
            Some(InquiryOutcome::Failed(crate::Error::InvalidState { .. }))
        ));
    }
}
