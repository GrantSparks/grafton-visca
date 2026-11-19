//! A Sony UDP completion stays observable when its ACK omitted the socket.
#![cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]

use std::time::Duration;

use grafton_visca::{
    command::zoom::Tele,
    config::{SessionConfig, TransportKind},
    operation::AppliedOnly,
    profiles::SonyFr7,
    types::IndividualCameraId,
    Error,
};
use grafton_visca_test_support::{
    facade_matrix,
    fake_camera::{frames, FakeCamera, ZOOM_TELE},
};

struct UdpCamera(FakeCamera);

impl UdpCamera {
    #[cfg(feature = "blocking")]
    fn blocking_wire(&self) -> grafton_visca_test_support::fake_camera::BlockingWire {
        self.0
            .blocking_wire()
            .with_transport_kind(TransportKind::Udp)
    }

    #[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
    fn async_wire(&self) -> grafton_visca_test_support::fake_camera::AsyncWire {
        self.0.async_wire().with_transport_kind(TransportKind::Udp)
    }
}

facade_matrix! {
    fn unnamed_sony_udp_ack_keeps_the_original_s2_completion_observable() {
        let fake = UdpCamera(FakeCamera::visca(|bytes, answer| {
            assert_eq!(bytes, ZOOM_TELE);
            answer.reply([0x90, 0x40, 0xff]);
        }));
        let session = open!(fake, SessionConfig::from_compile_time::<SonyFr7>().unwrap()).unwrap();
        let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).unwrap();
        let mut drive = wait!(camera.submit::<AppliedOnly, _>(&Tele)).unwrap();
        wait_for_reads!(fake.0, 1);
        assert!(matches!(wait!(drive.cancel()), Err(Error::CancellationUnconfirmed)));

        let writes = fake.0.writes();
        assert_eq!(writes.len(), 1);
        let sequence = frames::sony_sequence(&writes[0]).unwrap();
        fake.0.push(frames::sony_reply(sequence, &[0x90, 0x52, 0xff]));
        wait!(drive.applied_with_timeout(Duration::from_millis(120))).unwrap();
        assert_eq!(fake.0.take_payloads(), [ZOOM_TELE]);
        wait!(session.close()).unwrap();
    }
}
