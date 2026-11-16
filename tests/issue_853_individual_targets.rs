//! Adversarial public-boundary and seven-target cache checks for #853.

#![cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]

use grafton_visca::{
    config::SessionConfig,
    profiles::{ProfileSpec, PtzOpticsG2},
    state_cache::{StateEntry, StateKey},
    transport::{AddressingMode, SendSemantics, TransportConfig},
    types::CameraId,
    Error,
};
use grafton_visca_test_support::{facade_matrix, fake_camera::FakeCamera};

struct SerialCamera(FakeCamera);

impl SerialCamera {
    fn new() -> Self {
        Self(FakeCamera::new(|write, answer| {
            let camera = write[0] & 0x0f;
            let source = (camera + 8) << 4;
            answer
                .reply(vec![source, 0x41, 0xff])
                .reply(vec![source, 0x51, 0xff]);
        }))
    }

    #[cfg(feature = "blocking")]
    fn blocking_wire(&self) -> grafton_visca_test_support::fake_camera::BlockingWire {
        self.0
            .blocking_wire()
            .with_config(TransportConfig::for_serial())
            .with_semantics(SendSemantics::Stream)
            .with_addressing(AddressingMode::Serial)
    }

    #[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
    fn async_wire(&self) -> grafton_visca_test_support::fake_camera::AsyncWire {
        self.0
            .async_wire()
            .with_config(TransportConfig::for_serial())
            .with_semantics(SendSemantics::Stream)
            .with_addressing(AddressingMode::Serial)
    }
}

fn profile() -> ProfileSpec {
    ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile")
}

facade_matrix! {
    fn every_individual_target_keeps_its_own_cache_after_other_targets_finish() {
        let camera = SerialCamera::new();
        let mut config = SessionConfig::new(profile());
        for id in 2..=7 {
            config = config.with_target(CameraId::new(id).unwrap(), profile()).unwrap();
        }
        let session = open!(camera, config).expect("seven registered serial targets");
        for id in 1..=7 {
            let target = CameraId::new(id).unwrap();
            let view = session.camera::<PtzOpticsG2>(target).unwrap();
            assert_eq!(view.state_cache().target(), target);
            wait!(view.streaming().set_multicast(true)).expect("target-local applied effect");
            for check in 1..=7 {
                let cache = session
                    .camera::<PtzOpticsG2>(CameraId::new(check).unwrap())
                    .unwrap()
                    .state_cache();
                match cache.value(StateKey::StreamingMulticast) {
                    StateEntry::Set(value) if check <= id => assert_eq!(value, grafton_visca::state_cache::KnownState::StreamingMulticast(true)),
                    StateEntry::Unknown if check > id => {}
                    state => panic!("finishing target {id} changed target {check}: {state:?}"),
                }
            }
        }
        assert_eq!(camera.0.write_count(), 7);
        session.shutdown().unwrap();
    }

    fn broadcast_and_unregistered_views_fail_before_any_protocol_write() {
        let camera = SerialCamera::new();
        let session = open!(camera, SessionConfig::for_target(CameraId::CAMERA_7, profile()).unwrap())
            .expect("last individual target has an ordinary table slot");
        let error = session.camera::<PtzOpticsG2>(CameraId::BROADCAST).unwrap_err();
        assert!(matches!(error, Error::InvalidRequest { reason }
            if reason == "a session target must be an individual camera"));
        assert!(matches!(
            session.camera::<PtzOpticsG2>(CameraId::CAMERA_1),
            Err(Error::InvalidRequest { .. })
        ));
        #[cfg(any(feature = "async", feature = "blocking"))]
        {
            assert!(matches!(session.camera::<grafton_visca::profiles::AnyProfile>(CameraId::BROADCAST), Err(Error::InvalidRequest { .. })));
            assert!(matches!(session.camera::<grafton_visca::profiles::AnyProfile>(CameraId::CAMERA_1), Err(Error::InvalidRequest { .. })));
        }
        assert_eq!(camera.0.write_count(), 0);
        assert_eq!(session.camera::<PtzOpticsG2>(CameraId::CAMERA_7).unwrap().target(), CameraId::CAMERA_7);
        session.shutdown().unwrap();
    }
}
