//! Serial gets the same single-camera constructors as TCP and UDP (#650).
//!
//! `Connect::open_tcp_camera` / `open_udp_camera` and
//! `CameraConfig::open_camera` / `open_camera_async` bind the profile once and
//! hand back a `CameraSession<P>`. Serial had only the owner-backed
//! `open_serial`, so a single-camera serial program had to name its profile a
//! second time through `session.camera::<P>()`. These suites pin the serial
//! convenience: the same preflight, the same bind, the same feature gates.
//!
//! The compile-level half of the contract (the constructors exist with the
//! right bounds under the right features) lives in the `tests/api_contract`
//! `pass_serial_blocking` and `pass_serial_tokio` fixtures.

/// A path no host has a serial device on, so the constructor is exercised up
/// to the point of opening hardware and no further.
#[cfg(any(feature = "transport-serial", feature = "transport-serial-tokio"))]
const ABSENT_PORT: &str = "/dev/grafton-visca-issue-650-absent";

#[cfg(feature = "transport-serial")]
mod blocking_serial_single_camera {
    use grafton_visca::{
        blocking::{CameraConfig, Connect},
        profiles::PtzOpticsG2,
        Error,
    };

    #[test]
    fn a_non_serial_configuration_is_rejected_before_any_port_is_opened() {
        let error = CameraConfig::<PtzOpticsG2>::tcp("192.168.0.110")
            .open_serial_camera()
            .expect_err("a TCP configuration is not a serial one");

        assert!(
            matches!(&error, Error::InvalidState(reason) if reason.contains("serial transport configuration")),
            "unexpected preflight error: {error:?}"
        );
    }

    #[test]
    fn the_one_liner_reports_the_port_failure_instead_of_handing_back_a_camera() {
        let error = Connect::open_serial_camera::<PtzOpticsG2>(super::ABSENT_PORT, 9600)
            .expect_err("an absent serial port cannot produce a camera session");

        assert!(
            !matches!(&error, Error::InvalidState(_)),
            "an absent port is an I/O failure, not a configuration one: {error:?}"
        );
    }
}

#[cfg(feature = "transport-serial-tokio")]
mod tokio_serial_single_camera {
    use grafton_visca::{
        camera::{CameraConfig, Connect},
        profiles::PtzOpticsG2,
        runtime::TokioRuntime,
        Error,
    };

    #[tokio::test]
    async fn a_non_serial_configuration_is_rejected_before_any_port_is_opened() {
        let runtime = TokioRuntime::from_current().expect("Tokio runtime");
        let error = CameraConfig::<PtzOpticsG2>::tcp("192.168.0.110")
            .open_serial_camera_async(runtime)
            .await
            .expect_err("a TCP configuration is not a serial one");

        assert!(
            matches!(&error, Error::InvalidState(reason) if reason.contains("serial transport configuration")),
            "unexpected preflight error: {error:?}"
        );
    }

    #[tokio::test]
    async fn the_one_liner_reports_the_port_failure_instead_of_handing_back_a_camera() {
        let runtime = TokioRuntime::from_current().expect("Tokio runtime");
        let error =
            Connect::open_serial_camera::<PtzOpticsG2, _>(super::ABSENT_PORT, 9600, runtime)
                .await
                .expect_err("an absent serial port cannot produce a camera session");

        assert!(
            !matches!(&error, Error::InvalidState(_)),
            "an absent port is an I/O failure, not a configuration one: {error:?}"
        );
    }
}
