//! #828: `Session::open` checks the registered cameras against the cameras a
//! serial bus startup addressed, before any protocol I/O, on every facade and
//! for caller-built transports.
//!
//! Each case runs on the blocking facade and on the async facade under each
//! enabled runtime.

#![cfg(any(
    feature = "blocking",
    all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    )
))]

#[path = "common/fake_camera.rs"]
mod fake_camera;
#[macro_use]
#[path = "common/matrix.rs"]
mod matrix;

use grafton_visca::{
    profile::ProfileSpec,
    profiles::GenericVisca,
    transport::{AddressedBus, AddressingMode, SendSemantics, TransportConfig},
    CameraId, Error, SessionConfig,
};

#[cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
use fake_camera::AsyncWire;
#[cfg(feature = "blocking")]
use fake_camera::BlockingWire;
use fake_camera::FakeCamera;

const PORT: &str = "/dev/ttyVISCA0";

/// A caller-built serial connection whose startup addressed `addressed`
/// cameras. The camera counts writes so a rejected open can be shown to write
/// nothing.
struct Bus {
    camera: FakeCamera,
    addressed: Option<u8>,
}

impl Bus {
    fn new(addressed: Option<u8>) -> Self {
        Self {
            camera: FakeCamera::silent(),
            addressed,
        }
    }

    /// The blocking wire, in the shape `open!` asks its camera for.
    #[cfg(feature = "blocking")]
    fn blocking_wire(&self) -> BlockingWire {
        let wire = self
            .camera
            .blocking_wire()
            .with_config(TransportConfig::for_serial())
            .with_addressing(AddressingMode::Serial)
            .with_semantics(SendSemantics::Stream);
        match self.addressed {
            Some(cameras) => wire.with_addressed_bus(AddressedBus::new(PORT, cameras)),
            None => wire,
        }
    }

    /// The async wire, in the shape `open!` asks its camera for.
    #[cfg(all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    ))]
    fn async_wire(&self) -> AsyncWire {
        let wire = self
            .camera
            .async_wire()
            .with_config(TransportConfig::for_serial())
            .with_addressing(AddressingMode::Serial)
            .with_semantics(SendSemantics::Stream);
        match self.addressed {
            Some(cameras) => wire.with_addressed_bus(AddressedBus::new(PORT, cameras)),
            None => wire,
        }
    }
}

fn targets(ids: &[u8]) -> SessionConfig {
    let profile = ProfileSpec::from_compile_time::<GenericVisca>().expect("profile");
    let mut config =
        SessionConfig::for_target(CameraId::new(ids[0]).expect("camera id"), profile.clone())
            .expect("first target");
    for id in &ids[1..] {
        config = config
            .with_target(CameraId::new(*id).expect("camera id"), profile.clone())
            .expect("further target");
    }
    config
}

/// One open: the cameras the startup addressed, the registered ids, and
/// whether the open succeeds.
#[derive(Clone, Copy)]
struct Case {
    addressed: Option<u8>,
    ids: &'static [u8],
    opens: bool,
}

fn assert_outcome<T: std::fmt::Debug>(
    case: Case,
    result: Result<T, Error>,
    camera: &FakeCamera,
) -> Option<T> {
    let Case {
        addressed,
        ids,
        opens,
    } = case;
    match result {
        Ok(session) => {
            assert!(opens, "{addressed:?} / {ids:?} must not open");
            Some(session)
        }
        Err(error) => {
            assert!(!opens, "{addressed:?} / {ids:?} must open: {error:?}");
            let missing = ids
                .iter()
                .find(|id| Some(**id) > addressed)
                .expect("a missing camera");
            assert!(
                matches!(
                    &error,
                    Error::ConnectionFailed { addr, source, .. }
                        if addr == PORT
                            && source.kind() == std::io::ErrorKind::NotFound
                            && source.to_string() == format!(
                                "camera {missing} was not addressed by Address Set (chain reported {})",
                                addressed.expect("Address Set ran")
                            )
                ),
                "unexpected error {error:?}"
            );
            assert_eq!(camera.write_count(), 0, "rejected before any I/O");
            None
        }
    }
}

/// Opens a session for `case` on the running facade and checks the outcome.
macro_rules! check_case {
    ($case:expr) => {{
        let case: Case = $case;
        let bus = Bus::new(case.addressed);
        let result = open!(bus, targets(case.ids));
        if let Some(session) = assert_outcome(case, result, &bus.camera) {
            wait!(session.close()).expect("clean close");
        }
    }};
}

facade_matrix! {
    fn opens_when_the_chain_addressed_every_registered_camera() {
        check_case!(Case {
            addressed: Some(2),
            ids: &[1, 2],
            opens: true,
        });
    }

    // Cameras beyond the registered ones are allowed.
    fn opens_with_cameras_beyond_the_registered_ones() {
        check_case!(Case {
            addressed: Some(3),
            ids: &[1],
            opens: true,
        });
    }

    // A registered camera the chain did not address fails the open.
    fn refuses_a_registered_camera_the_chain_did_not_address() {
        check_case!(Case {
            addressed: Some(1),
            ids: &[1, 2],
            opens: false,
        });
    }

    fn refuses_when_no_registered_camera_was_addressed() {
        check_case!(Case {
            addressed: Some(2),
            ids: &[3],
            opens: false,
        });
    }

    // No Address Set ran: nothing to check.
    fn opens_when_no_address_set_ran() {
        check_case!(Case {
            addressed: None,
            ids: &[1, 2],
            opens: true,
        });
    }
}
