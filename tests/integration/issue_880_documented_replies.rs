//! Documented camera statuses and notifications remain distinguishable from corruption.

use grafton_visca::wire::WireValue;

use grafton_visca::{
    command::power::StatusInquiry, error::ErrorKind, request::Inquiry, types::PowerStatus,
    wire::Response, Error,
};

fn decode_power(frame: &[u8]) -> Result<PowerStatus, Error> {
    Response::parse(frame)?;
    if frame.get(1) != Some(&0x50) {
        return Err(Error::UnexpectedResponseType);
    }
    let payload = frame
        .get(2..frame.len().saturating_sub(1))
        .ok_or(Error::InvalidResponseFormat)?;
    StatusInquiry.decoder().decode(payload)
}

#[test]
fn power_status_decodes_all_documented_values_and_rejects_corruption() {
    for (byte, expected) in [
        (0x02, PowerStatus::On),
        (0x03, PowerStatus::Standby),
        (0x04, PowerStatus::CircuitError),
    ] {
        assert_eq!(PowerStatus::from_wire(byte).unwrap(), expected);
        assert_eq!(expected.to_wire(), byte);
        assert_eq!(expected.is_on(), byte == 0x02);
        assert_eq!(decode_power(&[0x90, 0x50, byte, 0xff]).unwrap(), expected);
    }
    for frame in [
        &[0x90, 0x50, 0x05, 0xff][..],
        &[0x90, 0x50, 0x02, 0x03, 0xff],
    ] {
        let error = decode_power(frame).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Protocol);
        assert!(matches!(
            error,
            Error::InvalidResponse { .. } | Error::InvalidResponseLength { .. }
        ));
    }
}

#[test]
fn network_change_is_a_notification_and_never_a_command_outcome() {
    let notification = Response::parse(&[0x90, 0x38, 0xff]).unwrap();
    assert!(matches!(notification, Response::NetworkChange));
    assert!(matches!(
        notification.into_result(),
        Err(Error::UnexpectedResponseType)
    ));
    assert!(matches!(
        Response::parse(&[0x90, 0x38, 0x00, 0xff]),
        Err(Error::InvalidResponse { .. })
    ));
}

#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
mod facades {
    use super::*;
    use grafton_visca::{config::SessionConfig, profiles::PtzOpticsG2};
    use grafton_visca_test_support::{facade_matrix, fake_camera::FakeCamera};

    fn camera(byte: u8) -> FakeCamera {
        FakeCamera::new(move |write, answer| {
            assert_eq!(write, [0x81, 0x09, 0x04, 0x00, 0xff]);
            answer.reply(vec![0x90, 0x50, byte, 0xff]);
        })
    }

    facade_matrix! {
        fn power_status_survives_the_owner_and_static_facades() {
            for expected in [PowerStatus::On, PowerStatus::Standby, PowerStatus::CircuitError] {
                let fake = camera(expected.to_wire());
                let session = open!(fake, SessionConfig::from_compile_time::<PtzOpticsG2>().unwrap()).unwrap();
                let view = session.camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1).unwrap();
                assert_eq!(wait!(view.power().status()).unwrap(), expected);
                assert_eq!(fake.write_count(), 1);
                session.shutdown().unwrap();
            }
        }
    }

    #[cfg(all(
        any(feature = "async", feature = "blocking"),
        any(feature = "runtime-tokio", feature = "runtime-smol")
    ))]
    async fn dynamic_power_status<E: grafton_visca::runtime::Executor>(executor: E) {
        for expected in [
            PowerStatus::On,
            PowerStatus::Standby,
            PowerStatus::CircuitError,
        ] {
            let fake = camera(expected.to_wire());
            let session = grafton_visca::r#async::Session::open(
                fake.async_wire(),
                &SessionConfig::from_compile_time::<PtzOpticsG2>().unwrap(),
                executor.clone(),
            )
            .await
            .unwrap();
            let view = session
                .camera::<grafton_visca::profiles::AnyProfile>(
                    grafton_visca::types::IndividualCameraId::CAMERA_1,
                )
                .unwrap();
            assert_eq!(view.power().status().await.unwrap(), expected);
            session.shutdown().unwrap();
        }
    }

    #[cfg(all(
        any(feature = "async", feature = "blocking"),
        any(feature = "runtime-tokio", feature = "runtime-smol")
    ))]
    grafton_visca_test_support::runtime_matrix!(dynamic_power_status);
}
