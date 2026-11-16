//! Documented camera statuses and notifications remain distinguishable from corruption.

use grafton_visca::wire::WireValue;

use grafton_visca::{
    command::{InquiryData, InquiryKind, Response},
    Error, ErrorKind, PowerStatus,
};

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
        assert!(matches!(
            Response::parse_with_type(&[0x90, 0x50, byte, 0xff], &InquiryKind::Power),
            Ok(Response::Inquiry(InquiryData::Power { status })) if status == expected
        ));
    }
    for frame in [
        &[0x90, 0x50, 0x05, 0xff][..],
        &[0x90, 0x50, 0x02, 0x03, 0xff],
    ] {
        let error = Response::parse_with_type(frame, &InquiryKind::Power).unwrap_err();
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
    use grafton_visca::{profiles::PtzOpticsG2, SessionConfig};
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
                let view = session.camera::<PtzOpticsG2>().unwrap();
                assert_eq!(wait!(view.power().state()).unwrap(), expected);
                assert_eq!(fake.write_count(), 1);
                session.shutdown().unwrap();
            }
        }
    }

    #[cfg(all(
        feature = "dyn-api",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    ))]
    async fn dynamic_power_status<E: grafton_visca::Executor>(executor: E) {
        for expected in [
            PowerStatus::On,
            PowerStatus::Standby,
            PowerStatus::CircuitError,
        ] {
            let fake = camera(expected.to_wire());
            let session = grafton_visca::Session::open(
                fake.async_wire(),
                SessionConfig::from_compile_time::<PtzOpticsG2>().unwrap(),
                executor.clone(),
            )
            .await
            .unwrap();
            let view = session.camera_dyn().unwrap();
            assert_eq!(view.power().state().await.unwrap(), expected);
            session.shutdown().unwrap();
        }
    }

    #[cfg(all(
        feature = "dyn-api",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    ))]
    grafton_visca_test_support::runtime_matrix!(dynamic_power_status);
}
