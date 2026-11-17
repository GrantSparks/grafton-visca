//! Synchronous admission and the legacy test input express one verdict.
use super::*;

fn fixture(case: &str, start: Instant) -> (ProtocolEngine, RuntimeRequest, AdmissionSlot, Instant) {
    let supported = CancellationPolicy::Supported;
    let mut engine = engine(EnvelopeKind::Raw, TransportKind::Datagram);
    let mut request = command(1, supported);
    let mut slot = AdmissionSlot::Ordinary;
    let mut now = start;
    match case {
        "admitted" => {}
        "terminal" => {
            engine.handle(Input::Shutdown(ShutdownReason::Explicit), now);
        }
        "invalid deadline" => {
            request.context_mut().timeout.completion = NonZeroDuration::new(Duration::MAX).unwrap();
        }
        "motion superseded" => {
            request.context_mut().submission_order = 1;
            request.context_mut().motion = Some(MotionEffect {
                axes: crate::AffectedAxes::ZOOM,
            });
            let turn = engine.begin_input_turn(now);
            engine.halt_in_turn(&turn, camera(1), crate::AffectedAxes::ZOOM, 1);
        }
        "ordinary capacity" => {
            engine.policy.capacity = std::num::NonZeroUsize::new(1).unwrap();
            let turn = engine.begin_input_turn(now);
            engine.admit_in_turn(&turn, request.clone(), slot).unwrap();
        }
        "nonurgent reserve" => {
            slot = AdmissionSlot::ControlReserve;
        }
        "full reserve" => {
            engine = reserved_engine();
            request = urgent_command(1, supported);
            slot = AdmissionSlot::ControlReserve;
            let turn = engine.begin_input_turn(now);
            for _ in 0..2 {
                engine.admit_in_turn(&turn, request.clone(), slot).unwrap();
            }
        }
        "unregistered target" => {
            request = command(7, supported);
        }
        "shape capacity" => {
            let turn = engine.begin_input_turn(now);
            for length in 1..=MAX_RAW_INQUIRY_SHAPE_CLASSES_PER_TARGET {
                engine
                    .admit_in_turn(
                        &turn,
                        inquiry_with_shape(1, PayloadShape::Bytes(length)),
                        slot,
                    )
                    .unwrap();
            }
            request = inquiry_with_shape(1, PayloadShape::Unknown);
        }
        "latched lane" => {
            engine = single_flight_raw_stream_engine();
            let (_, timed_out) = time_out_stream_inquiry(&mut engine, start);
            now = timed_out + Duration::from_millis(50);
            engine.advance(now);
            request = inquiry(1);
        }
        "cancellation conflict" => {
            request.context_mut().cancellation = CancellationPolicy::Unsupported;
        }
        "request identity exhausted" => {
            engine.seed_allocators(0, 1);
        }
        "duplicate identity refused" => {
            let turn = engine.begin_input_turn(now);
            engine.admit_in_turn(&turn, request.clone(), slot).unwrap();
            engine.seed_allocators(1, 1);
        }
        other => panic!("unknown admission case {other}"),
    }
    (engine, request, slot, now)
}

#[test]
fn synchronous_admission_and_test_shim_agree_on_every_verdict_path() {
    let start = Instant::now();
    for case in [
        "admitted",
        "terminal",
        "invalid deadline",
        "motion superseded",
        "ordinary capacity",
        "nonurgent reserve",
        "full reserve",
        "unregistered target",
        "shape capacity",
        "latched lane",
        "cancellation conflict",
        "request identity exhausted",
        "duplicate identity refused",
    ] {
        let (mut direct, request, slot, now) = fixture(case, start);
        let turn = direct.begin_input_turn(now);
        let verdict = direct.admit_in_turn(&turn, request, slot);
        let (mut shim, request, slot, now) = fixture(case, start);
        let turn = shim.begin_input_turn(now);
        let effects = shim.handle_in_turn(
            &turn,
            Input::Admit {
                ticket: AdmissionTicket(7),
                request,
                slot,
            },
        );
        assert_eq!(
            effects.len(),
            1,
            "{case}: admission emits only its test verdict"
        );
        let legacy = match effects.into_iter().next().unwrap() {
            Effect::Admitted { id, ticket } => {
                assert_eq!(ticket, AdmissionTicket(7));
                Ok(id)
            }
            Effect::AdmissionRejected { error, ticket } => {
                assert_eq!(ticket, AdmissionTicket(7));
                Err(error)
            }
            other => panic!("{case}: unrelated admission effect {other:?}"),
        };
        assert_eq!(
            verdict
                .map(|id| id.get())
                .map_err(|error| format!("{error:?}")),
            legacy
                .map(|id| id.get())
                .map_err(|error| format!("{error:?}")),
            "{case}"
        );
        assert_eq!(direct.active_len(), shim.active_len(), "{case}");
        direct.assert_invariants().unwrap();
        shim.assert_invariants().unwrap();
    }
}
