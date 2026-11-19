//! Startup RESET confirmation on Sony UDP and no-op startup on raw VISCA.

#![cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]
#![allow(clippy::expect_used)]

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use grafton_visca::{
    config::{SessionConfig, TransportKind},
    profiles::{GenericVisca, SonyFr7},
    transport::{AddressingMode, IoBounds, SendSemantics},
    types::{IndividualCameraId, PowerStatus},
    Error,
};
use grafton_visca_test_support::{facade_matrix, fake_camera::FakeCamera};

const RESET: [u8; 9] = [2, 0, 0, 1, 0, 0, 0, 0, 1];
const RESET_ACK: [u8; 9] = [2, 1, 0, 1, 0, 0, 0, 0, 1];
const SEQUENCE_ERROR: [u8; 10] = [2, 0, 0, 2, 0, 0, 0, 0, 0x0f, 1];
const MESSAGE_TYPE_ERROR: [u8; 10] = [2, 0, 0, 2, 0, 0, 0, 0, 0x0f, 2];
const POWER_INQUIRY: [u8; 13] = [1, 0x10, 0, 5, 0, 0, 0, 0, 0x81, 9, 4, 0, 0xff];
const POWER_REPLY: [u8; 12] = [1, 0x11, 0, 4, 0, 0, 0, 0, 0x90, 0x50, 2, 0xff];
const STALE_POWER_REPLY: [u8; 12] = [1, 0x11, 0, 4, 0, 0, 0, 0, 0x90, 0x50, 3, 0xff];
const STALE_REPLY_PREFIX_LEN: usize = 10;
const STARTUP_BUDGET: Duration = Duration::from_millis(60);

enum SonyWire {
    Udp,
    Stream,
}

struct SonyPeer(FakeCamera, SonyWire);

impl SonyPeer {
    fn bounds() -> IoBounds {
        IoBounds::default()
            .with_write_timeout(STARTUP_BUDGET)
            .expect("startup budget")
    }

    #[cfg(all(feature = "blocking", feature = "test-utils"))]
    fn blocking_wire(&self) -> grafton_visca_test_support::fake_camera::BlockingWire {
        let wire = self.0.blocking_wire().with_config(Self::bounds());
        match self.1 {
            SonyWire::Udp => wire.with_transport_kind(TransportKind::Udp),
            SonyWire::Stream => wire
                .with_semantics(SendSemantics::Stream)
                .with_addressing(AddressingMode::Ip),
        }
    }

    #[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
    fn async_wire(&self) -> grafton_visca_test_support::fake_camera::AsyncWire {
        let wire = self.0.async_wire().with_config(Self::bounds());
        match self.1 {
            SonyWire::Udp => wire.with_transport_kind(TransportKind::Udp),
            SonyWire::Stream => wire
                .with_semantics(SendSemantics::Stream)
                .with_addressing(AddressingMode::Ip),
        }
    }
}

fn sony_config() -> SessionConfig {
    SessionConfig::from_compile_time::<SonyFr7>().expect("FR7 config")
}

fn peer_with_stale_stream_prefix() -> FakeCamera {
    FakeCamera::new(|write, answer| {
        if write == RESET {
            answer.reply(
                [
                    RESET_ACK.as_slice(),
                    &STALE_POWER_REPLY[..STALE_REPLY_PREFIX_LEN],
                ]
                .concat(),
            );
        } else if write == POWER_INQUIRY {
            answer.reply(POWER_REPLY);
        }
    })
}

facade_matrix! {
    paused:

    fn delayed_confirmation_gates_sequence_zero_request() {
        let peer = FakeCamera::new(|write, answer| {
            if write == RESET {
                answer.reply(SEQUENCE_ERROR).reply(MESSAGE_TYPE_ERROR);
            } else if write == POWER_INQUIRY {
                answer.reply(POWER_REPLY);
            }
        });
        let responder = peer.clone();
        let delay = Duration::from_millis(24);
        let start = now!();
        after!(delay, move || {
            assert_eq!(responder.writes(), [RESET]);
            responder.push(RESET_ACK);
        });

        let config = sony_config().with_sony_sequence_reset_on_connect(true);
        let session = open!(SonyPeer(peer.clone(), SonyWire::Udp), config)
            .expect("positive RESET confirmation");
        assert_eq!(now!().duration_since(start), delay);
        assert_eq!(peer.writes(), [RESET]);
        assert_eq!(peer.read_count(), 3);
        let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).expect("FR7 view");
        assert_eq!(wait!(camera.power().status()).expect("first inquiry"), PowerStatus::On);
        assert_eq!(peer.writes(), [RESET.to_vec(), POWER_INQUIRY.to_vec()]);
        wait!(session.close()).expect("close Sony session");
        assert_eq!(peer.wires_dropped(), 1);
    }

    fn reset_confirmation_drains_partial_stale_stream_reply_before_sequence_zero() {
        let peer = peer_with_stale_stream_prefix();
        let responder = peer.clone();
        let delay = Duration::from_millis(24);
        let start = now!();
        after!(delay, move || {
            assert_eq!(responder.writes(), [RESET]);
            responder.push(&STALE_POWER_REPLY[STALE_REPLY_PREFIX_LEN..]);
        });
        let config = sony_config().with_sony_sequence_reset_on_connect(true);
        let session = open!(SonyPeer(peer.clone(), SonyWire::Stream), config)
            .expect("RESET confirms after the retained stream frame is discarded");
        assert_eq!(now!().duration_since(start), delay);
        assert_eq!(peer.writes(), [RESET]);
        assert_eq!(peer.read_count(), 2);
        let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).expect("FR7 view");
        assert_eq!(wait!(camera.power().status()).expect("first inquiry"), PowerStatus::On);
        assert_eq!(peer.writes(), [RESET.to_vec(), POWER_INQUIRY.to_vec()]);
        wait!(session.close()).expect("close Sony session");
    }

    fn partial_stream_reply_after_reset_ack_uses_the_same_deadline() {
        let peer = peer_with_stale_stream_prefix();
        let start = now!();
        let config = sony_config().with_sony_sequence_reset_on_connect(true);
        assert!(matches!(
            open!(SonyPeer(peer.clone(), SonyWire::Stream), config),
            Err(Error::Timeout { .. })
        ));
        assert_eq!(now!().duration_since(start), STARTUP_BUDGET);
        assert_eq!(peer.writes(), [RESET]);
        assert_eq!(peer.wires_dropped(), 1);
    }

    fn lost_reset_or_confirmation_expires_without_application_writes() {
        for delivered in [false, true] {
            let camera_counter_reset = Arc::new(AtomicBool::new(false));
            let counter = Arc::clone(&camera_counter_reset);
            let peer = FakeCamera::new(move |write, _| {
                if write == RESET && delivered {
                    // The camera applied RESET, but its confirmation was lost.
                    counter.store(true, Ordering::Release);
                }
            });
            let responder = peer.clone();
            after!(Duration::from_millis(45), move || {
                responder.push(SEQUENCE_ERROR);
                responder.push(MESSAGE_TYPE_ERROR);
                responder.push(RESET); // A control command is not confirmation.
                responder.push(POWER_REPLY); // An ordinary reply is not confirmation.
            });

            let start = now!();
            let config = sony_config().with_sony_sequence_reset_on_connect(true);
            assert!(matches!(
                open!(SonyPeer(peer.clone(), SonyWire::Udp), config),
                Err(Error::Timeout { .. })
            ));
            assert_eq!(now!().duration_since(start), STARTUP_BUDGET);
            assert_eq!(camera_counter_reset.load(Ordering::Acquire), delivered);
            assert_eq!(peer.writes(), [RESET]);
            assert_eq!(peer.wires_dropped(), 1);
        }
    }

    fn transport_closure_fails_startup_without_waiting_out_the_budget() {
        let peer = FakeCamera::new(|write, answer| {
            if write == RESET {
                answer.fault(Error::connection_closed(None));
            }
        });
        let start = now!();
        let config = sony_config().with_sony_sequence_reset_on_connect(true);
        assert!(matches!(
            open!(SonyPeer(peer.clone(), SonyWire::Udp), config),
            Err(Error::ConnectionClosed { .. })
        ));
        assert!(now!().duration_since(start) < STARTUP_BUDGET);
        assert_eq!(peer.writes(), [RESET]);
        assert_eq!(peer.wires_dropped(), 1);
    }

    fn defaults_skip_reset_and_raw_opt_in_is_rejected_before_io() {
        let sony = FakeCamera::new(|write, answer| {
            if write == POWER_INQUIRY {
                answer.reply(POWER_REPLY);
            }
        });
        let session =
            open!(SonyPeer(sony.clone(), SonyWire::Udp), sony_config()).expect("default Sony open");
        assert!(sony.writes().is_empty());
        let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).expect("FR7 view");
        assert_eq!(wait!(camera.power().status()).expect("default inquiry"), PowerStatus::On);
        assert_eq!(sony.writes(), [POWER_INQUIRY]);
        wait!(session.close()).expect("close Sony session");

        let config = SessionConfig::from_compile_time::<GenericVisca>().expect("raw config");
        let raw = FakeCamera::silent();
        let session = open!(raw, config.clone()).expect("default raw open");
        assert!(raw.writes().is_empty());
        wait!(session.close()).expect("close raw session");

        let raw = FakeCamera::silent();
        assert!(matches!(
            open!(raw, config.with_sony_sequence_reset_on_connect(true)),
            Err(Error::InvalidRequest { .. })
        ));
        assert!(raw.writes().is_empty());
        assert_eq!(raw.receive_calls(), 0);
        assert_eq!(raw.wires_dropped(), 1);
    }
}
