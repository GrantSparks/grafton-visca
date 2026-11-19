//! A slow write cannot consume the camera's minimum interval.

#![cfg(all(feature = "test-utils", any(feature = "blocking", feature = "async")))]

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use grafton_visca::{
    command::pan_tilt::Drive,
    config::{SessionConfig, TransportKind},
    operation::AppliedOnly,
    profiles::{PtzOpticsG2, SonyFr7},
    testing::testkit::ManualClock,
    types::{IndividualCameraId, PanSpeed, PanTiltDirection, PanTiltSpeed, TiltSpeed},
    Error,
};
use grafton_visca_test_support::fake_camera::{frames, FakeCamera, ZOOM_STOP};

const POWER_ON: &[u8] = &[0x81, 0x01, 0x04, 0x00, 0x02, 0xff];
const ZOOM_POSITION: &[u8] = &[0x81, 0x09, 0x04, 0x47, 0xff];

struct TimedCamera {
    peer: FakeCamera,
    arrivals: Arc<Mutex<Vec<Instant>>>,
}

impl TimedCamera {
    fn new(clock: &ManualClock, first_write_duration: Duration) -> Self {
        let clock = clock.clone();
        let arrivals = Arc::new(Mutex::new(Vec::new()));
        let received = Arc::clone(&arrivals);
        let peer = FakeCamera::visca(move |request, answer| {
            let mut times = received.lock().expect("arrival samples");
            if times.is_empty() {
                // Simulate time spent in transport before the peer sees the frame.
                clock.advance(first_write_duration);
            }
            times.push(clock.now());
            if request == ZOOM_POSITION {
                answer.reply(frames::inquiry_reply(&[0x01, 0x02, 0x03, 0x04]));
            } else {
                answer.reply(frames::ack(1)).reply(frames::complete(1));
            }
        });
        Self { peer, arrivals }
    }
}

#[derive(Clone, Copy, Debug)]
enum Call {
    PowerOn,
    ZoomPosition,
    ZoomStop,
}

impl Call {
    const fn packet(self) -> &'static [u8] {
        match self {
            Self::PowerOn => POWER_ON,
            Self::ZoomPosition => ZOOM_POSITION,
            Self::ZoomStop => ZOOM_STOP,
        }
    }
}

fn cases() -> impl Iterator<Item = ([Call; 2], Duration, Duration)> {
    [
        ([Call::PowerOn, Call::PowerOn], 100),
        ([Call::PowerOn, Call::ZoomStop], 100),
        ([Call::ZoomPosition, Call::ZoomPosition], 150),
        ([Call::ZoomPosition, Call::ZoomStop], 0),
    ]
    .into_iter()
    .flat_map(|(calls, spacing)| {
        [0, 40, 250].into_iter().map(move |write_ms| {
            (
                calls,
                Duration::from_millis(spacing),
                Duration::from_millis(write_ms),
            )
        })
    })
}

macro_rules! check_spacing {
    (await: [$($await:tt)*], $session:ident, $camera:ident, $calls:ident, $spacing:ident) => {{
        let view = $session
            .camera::<PtzOpticsG2>(IndividualCameraId::CAMERA_1)
            .expect("G2 camera");
        for call in $calls {
            match call {
                Call::PowerOn => view.power().on() $($await)* .expect("power on"),
                Call::ZoomPosition => {
                    view.zoom().position() $($await)* .expect("zoom position");
                }
                Call::ZoomStop => {
                    let mut stop = view.zoom().stop() $($await)* .expect("admitted zoom STOP");
                    stop.applied() $($await)* .expect("applied zoom STOP");
                }
            }
        }
        assert_eq!($camera.peer.writes(), $calls.map(Call::packet));
        let gap = {
            let times = $camera.arrivals.lock().expect("arrival times");
            times[1] - times[0]
        };
        if $spacing.is_zero() {
            assert!(gap < Duration::from_millis(100), "{:?}: {gap:?}", $calls);
        } else {
            assert!(gap >= $spacing, "{:?}: {gap:?}", $calls);
        }
        $session.close() $($await)* .expect("close session");
    }};
}

macro_rules! check_refusal {
    (await: [$($await:tt)*], $session:ident, $camera:ident, $clock:ident) => {{
        let view = $session
            .camera::<SonyFr7>(IndividualCameraId::CAMERA_1)
            .expect("FR7 camera");
        let fast_drive = Drive::new(
            PanTiltDirection::Right,
            PanTiltSpeed {
                pan: PanSpeed::new(50).expect("valid fine pan speed"),
                tilt: TiltSpeed::new(50).expect("valid fine tilt speed"),
            },
        );
        let before_drive = $clock.now();
        let mut drive = view.submit::<AppliedOnly, _>(&fast_drive) $($await)* .expect("admitted drive");
        assert!(matches!(drive.applied() $($await)* , Err(Error::TransmissionNotSent { .. })));
        assert_eq!($camera.peer.write_count(), 0);

        let mut stop = view.zoom().stop() $($await)* .expect("admitted zoom STOP");
        stop.applied() $($await)* .expect("applied zoom STOP");
        let wire = $camera.peer.writes();
        assert_eq!(wire.len(), 1);
        assert_eq!(wire[0], [0x01, 0x00, 0, 6, 0, 0, 0, 0, 0x81, 0x01, 0x04, 0x07, 0, 0xff]);
        let gap = $camera.arrivals.lock().expect("arrival time")[0] - before_drive;
        assert!(gap < Duration::from_millis(35), "FR7 STOP waited {gap:?}");
        $session.close() $($await)* .expect("close session");
    }};
}

#[cfg(feature = "blocking")]
#[test]
fn blocking() {
    for (calls, spacing, write_duration) in cases() {
        let clock = ManualClock::new();
        let participant = clock.enter();
        let camera = TimedCamera::new(&clock, write_duration);
        let session = clock
            .open_blocking_session(
                &participant,
                camera
                    .peer
                    .blocking_wire()
                    .with_transport_kind(TransportKind::Tcp)
                    .on_clock(&clock),
                SessionConfig::from_compile_time::<PtzOpticsG2>().expect("G2 config"),
            )
            .expect("G2 session");
        check_spacing!(await: [], session, camera, calls, spacing);
    }

    let clock = ManualClock::new();
    let participant = clock.enter();
    let camera = TimedCamera::new(&clock, Duration::ZERO);
    let session = clock
        .open_blocking_session(
            &participant,
            camera
                .peer
                .blocking_wire()
                .with_transport_kind(TransportKind::Udp)
                .on_clock(&clock),
            SessionConfig::from_compile_time::<SonyFr7>().expect("FR7 config"),
        )
        .expect("FR7 session");
    check_refusal!(await: [], session, camera, clock);
}

#[cfg(feature = "async")]
#[test]
fn async_manual_clock() {
    use grafton_visca::{r#async::Session, testing::testkit::ManualClockExecutor};

    for (calls, spacing, write_duration) in cases() {
        let clock = ManualClock::new();
        let executor = ManualClockExecutor::local(&clock);
        executor.run_until(async {
            let camera = TimedCamera::new(&clock, write_duration);
            let session = Session::open(
                camera
                    .peer
                    .async_wire()
                    .with_transport_kind(TransportKind::Tcp),
                &SessionConfig::from_compile_time::<PtzOpticsG2>().expect("G2 config"),
                executor.clone(),
            )
            .await
            .expect("G2 session");
            check_spacing!(await: [.await], session, camera, calls, spacing);
        });
    }

    let clock = ManualClock::new();
    let executor = ManualClockExecutor::local(&clock);
    executor.run_until(async {
        let camera = TimedCamera::new(&clock, Duration::ZERO);
        let session = Session::open(
            camera
                .peer
                .async_wire()
                .with_transport_kind(TransportKind::Udp),
            &SessionConfig::from_compile_time::<SonyFr7>().expect("FR7 config"),
            executor.clone(),
        )
        .await
        .expect("FR7 session");
        check_refusal!(await: [.await], session, camera, clock);
    });
}
