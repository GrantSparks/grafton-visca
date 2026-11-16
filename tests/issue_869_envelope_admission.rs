//! A profile's unsupported envelope payload fails before admission and I/O.

#![cfg(all(
    feature = "dyn-api",
    any(
        feature = "runtime-tokio",
        all(
            feature = "test-utils",
            any(feature = "blocking", feature = "runtime-smol")
        )
    )
))]
#![allow(clippy::expect_used)]

use grafton_visca::{
    camera::TransportKind, capabilities::Capabilities, profiles::GenericVisca, raw, CameraId,
    Error, OperationalTuning, ProfileEnvelope, ProfileSpec, RetryClass, SessionConfig,
    SessionStatus, TransportCompatibility,
};
use grafton_visca_test_support::{
    facade_matrix,
    fake_camera::{self, FakeCamera},
};

struct CameraWire {
    camera: FakeCamera,
    kind: TransportKind,
}

impl CameraWire {
    #[cfg(all(feature = "blocking", feature = "test-utils"))]
    fn blocking_wire(&self) -> fake_camera::BlockingWire {
        self.camera.blocking_wire().with_transport_kind(self.kind)
    }

    #[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
    fn async_wire(&self) -> fake_camera::AsyncWire {
        self.camera.async_wire().with_transport_kind(self.kind)
    }
}

fn sony_profile() -> ProfileSpec {
    let baseline = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic profile");
    let mut capabilities = Capabilities::from_profile::<GenericVisca>();
    capabilities.profile_id = None;
    ProfileSpec::builder(capabilities)
        .transports(TransportCompatibility::new(Some(5678), Some(52381), false))
        .envelope(ProfileEnvelope::SonyEncapsulated)
        .timing(baseline.timing())
        .maximum_command_sockets(baseline.maximum_command_sockets())
        .supports_operation_complete(baseline.supports_operation_complete())
        .supports_command_cancel(baseline.supports_command_cancel())
        .position_inquiries(baseline.position_inquiries())
        .build()
        .expect("Sony-envelope runtime profile")
}

facade_matrix! {
    paused:

    fn rejected_envelope_payload_does_not_poison_stream_or_datagram() {
        for kind in [TransportKind::Tcp, TransportKind::Udp] {
            let fake = FakeCamera::visca(|_, answer| {
                answer.reply(fake_camera::frames::ack(1)).reply(fake_camera::frames::complete(1));
            });
            let wire = CameraWire { camera: fake.clone(), kind };
            let session = open!(wire, SessionConfig::new(sony_profile())
                .with_tuning(OperationalTuning::new())).expect("session");
            let view = session.camera_dyn().expect("runtime camera");
            let mut bytes = vec![0; 17];
            bytes[0] = 0x80 | CameraId::CAMERA_1.value();
            bytes[16] = 0xff;
            let oversized = raw::Plain::new(bytes, grafton_visca::CommandCategory::Quick,
                RetryClass::Never, grafton_visca::SubmissionClass::Normal).expect("bounded raw value");
            assert!(matches!(wait!(view.execute(&oversized)), Err(Error::InvalidRequest(_))));
            let metrics = wait!(session.metrics()).expect("metrics");
            assert_eq!(metrics.admitted, 0);
            assert_eq!(metrics.write_failures, 0);
            assert_eq!(metrics.session, SessionStatus::Running);
            assert_eq!(fake.write_count(), 0);

            let valid = raw::Plain::new([0x81, 1, 4, 0, 2, 0xff], grafton_visca::CommandCategory::Quick,
                RetryClass::Never, grafton_visca::SubmissionClass::Normal).expect("ordinary command");
            wait!(view.execute(&valid)).expect("later command applies on the same session");
            let metrics = wait!(session.metrics()).expect("metrics");
            assert_eq!(metrics.admitted, 1);
            assert_eq!(metrics.write_failures, 0);
            assert_eq!(metrics.session, SessionStatus::Running);
            wait!(session.close()).expect("close session");
        }
    }
}
