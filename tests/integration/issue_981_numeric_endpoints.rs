//! Endpoint syntax is checked before each facade creates a transport. The
//! blocking and async constructor signatures differ, so construction is tested
//! directly and the async scenario is shared across both runtime adapters.

#![cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]

use grafton_visca::{
    config::{SessionConfig, TransportOptions},
    profiles::{ProfileId, PtzOpticsG2},
    Error,
};

fn assert_noncanonical<T>(result: grafton_visca::Result<T>) {
    assert!(matches!(
        result,
        Err(Error::InvalidAddress { reason, .. })
            if reason == "IPv4 literals must be canonical dotted-quad without leading zeros"
    ));
}

#[cfg(feature = "blocking")]
#[test]
fn blocking_constructors_reject_numeric_hosts() {
    use grafton_visca::blocking::{CameraSession, Session};

    for address in ["192.168.1.010:1259", "127.1:1259", "0x7f000001:1259"] {
        assert_noncanonical(CameraSession::<PtzOpticsG2>::connect_tcp(address));
        assert_noncanonical(CameraSession::<PtzOpticsG2>::connect_udp(address));
        for transport in [
            TransportOptions::tcp(address),
            TransportOptions::udp(address),
        ] {
            let config = SessionConfig::new(ProfileId::PtzOpticsG2.spec().expect("profile"))
                .with_transport(transport);
            assert_noncanonical(Session::connect(&config));
        }
    }
}

#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
async fn async_constructors_reject_numeric_hosts<R: grafton_visca::runtime::Runtime>(runtime: R) {
    use grafton_visca::r#async::{CameraSession, Session};

    for address in ["192.168.1.010:1259", "127.1:1259", "0x7f000001:1259"] {
        assert_noncanonical(
            CameraSession::<PtzOpticsG2>::connect_tcp(address, runtime.clone()).await,
        );
        assert_noncanonical(
            CameraSession::<PtzOpticsG2>::connect_udp(address, runtime.clone()).await,
        );
        for transport in [
            TransportOptions::tcp(address),
            TransportOptions::udp(address),
        ] {
            let config = SessionConfig::new(ProfileId::PtzOpticsG2.spec().expect("profile"))
                .with_transport(transport);
            assert_noncanonical(Session::connect(&config, runtime.clone()).await);
        }
    }
}

#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
grafton_visca_test_support::runtime_matrix!(async_constructors_reject_numeric_hosts);
