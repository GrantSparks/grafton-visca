//! Decoder payload shapes preserve raw inquiry attribution and availability.

#![cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::time::Duration;

use grafton_visca::{
    config::{OperationalTuning, SessionConfig},
    error::{Certainty, ErrorKind, FailureContext, FailureStage},
    observability::{DiagnosticEvent, DiagnosticIgnoreReason, MetricsSnapshot},
    profiles::{ProfileSpec, PtzOpticsG2},
    transport::SendSemantics,
    types::{AffectedAxes, FocusMode, MotionQuery, ZoomPosition},
    Error,
};
use grafton_visca_test_support::{
    facade_matrix,
    fake_camera::{self, frames, FakeCamera},
};

const MODE: [u8; 5] = [0x81, 0x09, 0x04, 0x38, 0xff];
const ZOOM: [u8; 5] = [0x81, 0x09, 0x04, 0x47, 0xff];
const ZOOM_DATA: [u8; 4] = [0x01, 0x02, 0x03, 0x04];

fn config() -> SessionConfig {
    SessionConfig::from_compile_time::<PtzOpticsG2>()
        .expect("G2 profile")
        .with_tuning(
            OperationalTuning::new()
                .with_retry_limit(0)
                .with_inquiry_timeout(Duration::from_secs(2)),
        )
}

fn profile() -> ProfileSpec {
    ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile")
}

fn zoom() -> ZoomPosition {
    ZoomPosition::new(0x1234).expect("documented G2 zoom position")
}

fn inquiry_timeout(error: Error) {
    assert!(matches!(error, Error::InquiryFailed { .. }), "{error:?}");
    assert_eq!(error.kind(), ErrorKind::Timeout);
    assert_eq!(
        error.failure_context(),
        Some(FailureContext::new(
            FailureStage::Terminal,
            Certainty::FailedConclusively,
        ))
    );
    assert!(!error.requires_new_session());
}

fn one_mismatch(metrics: MetricsSnapshot, next_event: impl FnMut() -> Option<DiagnosticEvent>) {
    assert_eq!(metrics.ignored_reply_shape_mismatches, 1);
    assert_eq!(metrics.ignored_malformed_frames, 0);
    assert_eq!(metrics.dropped_diagnostic_events, 0);
    let mismatches = std::iter::from_fn(next_event)
        .filter(|event| {
            matches!(
                event,
                DiagnosticEvent::Ignored {
                    reason: DiagnosticIgnoreReason::ReplyShapeMismatch,
                    ..
                }
            )
        })
        .count();
    assert_eq!(mismatches, 1);
}

struct Rig {
    peer: FakeCamera,
    semantics: SendSemantics,
}

impl Rig {
    /// The first one-byte mode reply never arrives. Later mode queries answer
    /// MANUAL so a new one-byte query proves a crossing position reply retired
    /// the old terminal debt by write order.
    fn missing_mode() -> Self {
        let mut mode_queries = 0;
        Self {
            semantics: SendSemantics::Stream,
            peer: FakeCamera::new(move |request, answer| {
                if request == MODE {
                    mode_queries += 1;
                    if mode_queries > 1 {
                        answer.reply(frames::inquiry_reply(&[0x03]));
                    }
                } else if request == ZOOM {
                    answer.reply(frames::inquiry_reply(&ZOOM_DATA));
                } else {
                    answer.reply(frames::ack(1)).reply(frames::complete(1));
                }
            }),
        }
    }

    /// An old one-byte reply arrives only once a four-nibble query is live.
    fn stale_datagram() -> Self {
        Self {
            semantics: SendSemantics::Datagram,
            peer: FakeCamera::new(|request, answer| {
                if request == ZOOM {
                    answer
                        .reply(frames::inquiry_reply(&[0x03]))
                        .reply(frames::inquiry_reply(&ZOOM_DATA));
                }
            }),
        }
    }

    fn undocumented_datagram() -> Self {
        let mut sent_undocumented = false;
        Self {
            semantics: SendSemantics::Datagram,
            peer: FakeCamera::new(move |request, answer| {
                if request == ZOOM && !sent_undocumented {
                    sent_undocumented = true;
                    // Length alone admits this frame; the Nibbles contract
                    // must also reject its high data bits before attribution.
                    answer.reply(frames::inquiry_reply(&[0x10, 0, 0, 0]));
                } else if request == MODE {
                    answer.reply(frames::inquiry_reply(&[0x03]));
                }
            }),
        }
    }

    #[cfg(any(
        feature = "runtime-tokio",
        all(feature = "blocking", feature = "test-utils")
    ))]
    fn ordered(semantics: SendSemantics, zoom_first: bool) -> Self {
        let mut mode_written = false;
        let mut zoom_written = false;
        let mut answered = false;
        Self {
            semantics,
            peer: FakeCamera::new(move |request, answer| {
                mode_written |= request == MODE;
                zoom_written |= request == ZOOM;
                if mode_written && zoom_written && !answered {
                    answered = true;
                    let mode = frames::inquiry_reply(&[0x03]);
                    let zoom = frames::inquiry_reply(&ZOOM_DATA);
                    if zoom_first {
                        answer.reply(zoom).reply(mode);
                    } else {
                        answer.reply(mode).reply(zoom);
                    }
                }
            }),
        }
    }

    #[cfg(all(feature = "blocking", feature = "test-utils"))]
    fn blocking_wire(&self) -> fake_camera::BlockingWire {
        self.peer.blocking_wire().with_semantics(self.semantics)
    }

    #[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
    fn async_wire(&self) -> fake_camera::AsyncWire {
        self.peer.async_wire().with_semantics(self.semantics)
    }
}

facade_matrix! {
    paused:

    fn motion_observation_crosses_a_missing_one_byte_mode_reply() {
        for latched in [false, true] {
            let wire = Rig::missing_mode();
            let session = open!(wire, config()).expect("raw stream session");
            let view = session.camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1).expect("G2 view");
            inquiry_timeout(wait!(view.inquire(&grafton_visca::command::focus::ModeInquiry)).expect_err("mode reply skipped"));
            if latched {
                pause!(profile().timing().ambiguity_timeout() + profile().timing().inquiry_spacing());
            }
            assert!(!wait!(view.motion().is_moving(
                MotionQuery::new(AffectedAxes::ZOOM).with_window(Duration::from_millis(50))
            )).expect("four-nibble position polls remain available"));
            assert_eq!(wait!(view.inquire(&grafton_visca::command::focus::ModeInquiry)).expect("new mode inquiry after crossing"), FocusMode::Manual);
            wait!(session.close()).expect("close");
        }
    }

    fn settlement_polling_crosses_a_missing_one_byte_mode_reply() {
        use grafton_visca_test_support::profile_fixtures::DirectZoomOnlyTypedSupport as Polled;
        for latched in [false, true] {
            let wire = Rig::missing_mode();
            let profile = ProfileSpec::from_compile_time::<Polled>().expect("polled zoom profile");
            assert!(!profile.supports_operation_complete(), "the fixture must poll positions");
            let latch_delay = profile.timing().ambiguity_timeout()
                + profile.timing().inquiry_spacing()
                + Duration::from_millis(20);
            let config = SessionConfig::new(profile).with_tuning(
                OperationalTuning::new().with_retry_limit(0).with_inquiry_timeout(Duration::from_secs(2))
            );
            let session = open!(wire, config).expect("raw stream session");
            let view = session.camera::<Polled>(grafton_visca::types::IndividualCameraId::CAMERA_1).expect("polled zoom view");
            // Finish the command first: its ACK must not retire the mode debt
            // before this test asks settlement to cross it with a data reply.
            let mut operation = wait!(view.zoom().set_position(zoom())).expect("zoom operation");
            wait!(operation.applied()).expect("zoom command applied");
            inquiry_timeout(wait!(view.inquire(&grafton_visca::command::focus::ModeInquiry)).expect_err("mode reply skipped"));
            if latched {
                pause!(latch_delay);
            }
            assert_eq!(wire.peer.writes().iter().filter(|request| request.as_slice() == ZOOM).count(), 0,
                "the pending mode debt must precede every settlement position poll");
            wait!(operation.settled()).expect("settlement position polls remain available");
            assert!(wire.peer.writes().iter().filter(|request| request.as_slice() == ZOOM).count() >= 2,
                "settlement must observe two position samples rather than use command completion");
            assert_eq!(wait!(view.inquire(&grafton_visca::command::focus::ModeInquiry)).expect("new mode inquiry after settlement"), FocusMode::Manual);
            wait!(session.close()).expect("close");
        }
    }

    fn a_stale_one_byte_datagram_cannot_finish_a_four_nibble_query() {
        let wire = Rig::stale_datagram();
        let session = open!(wire, config()).expect("raw datagram session");
        let diagnostics = wait!(session.subscribe_diagnostics(64)).expect("diagnostics");
        let view = session.camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1).expect("G2 view");
        inquiry_timeout(wait!(view.inquire(&grafton_visca::command::focus::ModeInquiry)).expect_err("old inquiry times out"));
        pause!(profile().timing().inquiry_spacing() + Duration::from_millis(20));
        assert_eq!(wait!(view.inquire(&grafton_visca::command::zoom::PositionInquiry)).expect("own four-nibble reply"), zoom());
        one_mismatch(wait!(session.metrics()).expect("metrics"), || diagnostics.try_recv());
        wait!(session.close()).expect("close");
    }

    fn an_undocumented_payload_is_visible_and_its_query_still_times_out() {
        let wire = Rig::undocumented_datagram();
        let session = open!(wire, config()).expect("raw datagram session");
        let diagnostics = wait!(session.subscribe_diagnostics(64)).expect("diagnostics");
        let view = session.camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1).expect("G2 view");
        inquiry_timeout(wait!(view.inquire(&grafton_visca::command::zoom::PositionInquiry)).expect_err("no structurally legal answer"));
        one_mismatch(wait!(session.metrics()).expect("metrics"), || diagnostics.try_recv());
        assert_eq!(wait!(view.inquire(&grafton_visca::command::focus::ModeInquiry)).expect("disjoint inquiry remains usable"), FocusMode::Manual);
        wait!(session.close()).expect("close");
    }
}

#[cfg(all(feature = "blocking", feature = "test-utils"))]
#[test]
fn blocking_disjoint_live_queries_receive_their_own_values_in_both_orders() {
    use grafton_visca::testing::testkit::ManualClock;

    for semantics in [SendSemantics::Stream, SendSemantics::Datagram] {
        for zoom_first in [false, true] {
            let clock = ManualClock::new();
            let participant = clock.enter();
            let wire = Rig::ordered(semantics, zoom_first);
            let session = clock
                .open_blocking_session(
                    &participant,
                    wire.blocking_wire().on_clock(&clock),
                    config(),
                )
                .expect("virtual raw session");
            let mode_view = session
                .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
                .expect("G2 view");
            let zoom_view = mode_view.clone();
            let mode =
                clock.spawn(move || mode_view.inquire(&grafton_visca::command::focus::ModeInquiry));
            let position = clock
                .spawn(move || zoom_view.inquire(&grafton_visca::command::zoom::PositionInquiry));
            assert_eq!(
                mode.join().expect("mode caller").expect("mode reply"),
                FocusMode::Manual
            );
            assert_eq!(
                position
                    .join()
                    .expect("position caller")
                    .expect("position reply"),
                zoom()
            );
            assert_eq!(
                wire.peer.write_count(),
                2,
                "both inquiries must be live before either reply"
            );
            session.close().expect("close");
        }
    }
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test(start_paused = true)]
async fn tokio_disjoint_live_queries_receive_their_own_values_in_both_orders() {
    use grafton_visca::{r#async::Session, runtime::TokioRuntime};

    for semantics in [SendSemantics::Stream, SendSemantics::Datagram] {
        for zoom_first in [false, true] {
            let runtime = TokioRuntime::from_current().expect("Tokio runtime");
            let wire = Rig::ordered(semantics, zoom_first);
            let session = Session::open(wire.async_wire(), &config(), runtime)
                .await
                .expect("virtual raw session");
            let view = session
                .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
                .expect("G2 view");
            let (mode, position) = tokio::join!(
                view.inquire(&grafton_visca::command::focus::ModeInquiry),
                view.inquire(&grafton_visca::command::zoom::PositionInquiry)
            );
            assert_eq!(mode.expect("mode reply"), FocusMode::Manual);
            assert_eq!(position.expect("position reply"), zoom());
            assert_eq!(
                wire.peer.write_count(),
                2,
                "both inquiries must be live before either reply"
            );
            session.close().await.expect("close");
        }
    }
}
