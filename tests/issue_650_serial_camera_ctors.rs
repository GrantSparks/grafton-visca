//! Serial retains the multi-target session construction path (#650, #729).
//!
//! TCP and UDP represent one target and return a profile-bound
//! `CameraSession<P>`. A serial camera uses the same shape; multi-camera
//! construction belongs to `Session::connect`. These suites pin the serial
//! configuration, preflight, and feature gates.
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
        config::{CameraConfig, TransportKind, TransportOptions},
        profiles::PtzOpticsG2,
        Error,
    };

    #[test]
    fn typed_serial_configuration_selects_the_serial_transport() {
        let config = CameraConfig::<PtzOpticsG2>::serial(super::ABSENT_PORT, 9600);
        assert_eq!(config.transport().kind(), TransportKind::Serial);
        assert!(
            matches!(config.transport(), TransportOptions::Serial { port, baud_rate: 9600, .. } if port == super::ABSENT_PORT)
        );
    }

    #[test]
    fn the_one_liner_reports_the_port_failure_instead_of_handing_back_a_session() {
        let error = grafton_visca::blocking::CameraSession::<PtzOpticsG2>::connect_serial(
            super::ABSENT_PORT,
            9600,
        )
        .expect_err("an absent serial port cannot produce a session");

        assert!(
            !matches!(&error, Error::InvalidState { .. }),
            "an absent port is an I/O failure, not a configuration one: {error:?}"
        );
    }
}

#[cfg(feature = "transport-serial-tokio")]
mod tokio_serial_single_camera {
    use grafton_visca::{
        config::{CameraConfig, TransportKind, TransportOptions},
        profiles::PtzOpticsG2,
        runtime::TokioRuntime,
        Error,
    };

    #[test]
    fn typed_serial_configuration_selects_the_serial_transport() {
        let config = CameraConfig::<PtzOpticsG2>::serial(super::ABSENT_PORT, 9600);
        assert_eq!(config.transport().kind(), TransportKind::Serial);
        assert!(
            matches!(config.transport(), TransportOptions::Serial { port, baud_rate: 9600, .. } if port == super::ABSENT_PORT)
        );
    }

    #[tokio::test]
    async fn the_one_liner_reports_the_port_failure_instead_of_handing_back_a_session() {
        let runtime = TokioRuntime::from_current().expect("Tokio runtime");
        let error = grafton_visca::r#async::CameraSession::<PtzOpticsG2>::connect_serial(
            super::ABSENT_PORT,
            9600,
            runtime,
        )
        .await
        .expect_err("an absent serial port cannot produce a session");

        assert!(
            !matches!(&error, Error::InvalidState { .. }),
            "an absent port is an I/O failure, not a configuration one: {error:?}"
        );
    }
}
