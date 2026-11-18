// Compile the actual private coordinator without exposing a test-only public
// library API. The engine and release-proof stubs supply only its input types;
// the vacancy constructors, loans, plan and fill methods are production source.
macro_rules! owner_coordinator_contract {
    ($body:block) => {
        #[allow(dead_code)]
        mod runtime {
            pub mod engine {
                #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
                pub struct RawCorrelationReleaseSet;
                impl RawCorrelationReleaseSet {
                    pub fn is_empty(self) -> bool { false }
                }
            }
            pub mod owner {
                pub mod turn {
                    use crate::runtime::engine::RawCorrelationReleaseSet;
                    use std::time::Instant;
                    #[derive(Debug, Clone, Copy, Default)]
                    pub struct RawReleaseTurn;
                    impl RawReleaseTurn {
                        pub fn observe(&mut self, _: RawCorrelationReleaseSet) -> Option<RawCorrelationReleaseSet> { None }
                        pub fn latched(self) -> Option<RawCorrelationReleaseSet> { None }
                        pub fn is_pending(self) -> bool { false }
                        pub fn is_fenced(self, _: RawCorrelationReleaseSet) -> bool { false }
                        pub fn await_until(self) -> Option<Instant> { None }
                        pub fn probe_scope(self) -> Option<RawCorrelationReleaseSet> { None }
                        pub fn fence_no_input(&mut self, _: RawCorrelationReleaseSet) {}
                        pub fn clear_fence(&mut self) {}
                        pub fn complete(&mut self) {}
                    }
                    pub mod coordinator {
                        include!(concat!(env!("GRAFTON_VISCA_SOURCE_ROOT"), "/src/runtime/owner/turn/coordinator.rs"));
                    }
                }
                fn vacancy(turn: turn::coordinator::PlannedTurn<'_, u32>) -> turn::coordinator::VacantSlot<'_, u32> {
                    use turn::coordinator::*;
                    let selection = Selection { phase: SourcePhase::ReceiveFirst, receive: ReceiveArm::Poll,
                        timer: TimerArm::Engine { at: None, due: false }, boundaries_eligible: true,
                        timer_precedes_control: false };
                    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
                    match turn.poll_in_order(selection, &mut context,
                        |_, _, _| std::task::Poll::<()>::Pending,
                        |_, _| std::task::Poll::Ready(BoundaryPoll::Boundary(7))) {
                        SelectionPoll::Ready(SelectedInput::Boundary { vacancy, .. }) => vacancy,
                        _ => unreachable!("a vacant selection polls its boundary"),
                    }
                }
                pub fn check() $body
            }
        }
        fn main() { runtime::owner::check(); }
    };
}
