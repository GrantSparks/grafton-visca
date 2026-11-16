//! Owner-facade coverage for the isolated typed raw request classes.
//!
//! The blocking facade has one scenario. The async facade runs the same
//! scenario under each enabled runtime, as the cases
//! `async_tests::owner_admits_all_typed_raw_classes::tokio` and `::smol`.

#![cfg(any(
    feature = "blocking",
    all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    )
))]

use grafton_visca_test_support::fake_camera;

use grafton_visca::{
    profiles::ProfileSpec,
    raw,
    request::{ResponseDecoder, RetryClass},
    types::AffectedAxes,
    Error,
};

use fake_camera::{frames, FakeCamera};

/// A camera that answers an inquiry with data `[1, 2]` and every other
/// request with ACK and completion on socket 1.
fn raw_camera() -> FakeCamera {
    FakeCamera::new(|write, answer| {
        if write.get(1) == Some(&0x09) {
            answer.reply(frames::inquiry_reply(&[0x01, 0x02]));
        } else {
            answer.reply(frames::ack(1)).reply(frames::complete(1));
        }
    })
}

/// The wire bytes the owner writes for the five requests the scenarios submit.
fn expected_writes(
    plain: &raw::PlainRequest,
    inquiry: &raw::InquiryRequest<u8>,
    targeted: &raw::TargetedRequest,
    applied: &raw::AppliedOnlyRequest,
    iris_reset: &grafton_visca::command::exposure::IrisReset,
) -> Vec<Vec<u8>> {
    vec![
        plain.bytes().to_vec(),
        inquiry.bytes().to_vec(),
        targeted.bytes().to_vec(),
        applied.bytes().to_vec(),
        {
            let mut bytes = [0_u8; 8];
            let length = grafton_visca::request::Request::write_into(
                iris_reset,
                grafton_visca::types::CameraId::CAMERA_1,
                &mut bytes,
            )
            .expect("typed iris wire");
            bytes[..length].to_vec()
        },
    ]
}

fn decode_first(payload: &[u8]) -> grafton_visca::Result<u8> {
    payload
        .first()
        .copied()
        .ok_or_else(|| Error::InvalidRequest {
            reason: "empty raw inquiry payload".into(),
        })
}

fn raw_values() -> (
    raw::PlainRequest,
    raw::InquiryRequest<u8>,
    raw::TargetedRequest,
    raw::AppliedOnlyRequest,
) {
    let plain = raw::PlainRequest::new(
        [0x81, 0x01, 0x01, 0xff],
        grafton_visca::profiles::CommandTimeoutClass::Quick,
        RetryClass::Never,
        grafton_visca::request::SubmissionClass::Normal,
    )
    .expect("plain frame");
    let inquiry = raw::InquiryRequest::new(
        [0x81, 0x09, 0x01, 0xff],
        ResponseDecoder::from_fn(decode_first),
        RetryClass::Inquiry,
        grafton_visca::request::SubmissionClass::Normal,
    )
    .expect("inquiry frame");
    let targeted = raw::TargetedRequest::new(
        [0x81, 0x01, 0x06, 0xff],
        AffectedAxes::PAN_TILT,
        grafton_visca::profiles::CommandTimeoutClass::Movement,
        RetryClass::Movement,
        grafton_visca::request::SubmissionClass::User,
    )
    .expect("targeted frame");
    // A raw applied-only operation classifies itself `User`, the highest lane a
    // raw caller may select; the urgent safety lane is owner-only (#679).
    let applied = raw::AppliedOnlyRequest::new(
        [0x81, 0x01, 0x07, 0xff],
        AffectedAxes::ZOOM,
        grafton_visca::profiles::CommandTimeoutClass::Quick,
        RetryClass::Never,
        grafton_visca::request::SubmissionClass::User,
    )
    .expect("applied-only frame");
    (plain, inquiry, targeted, applied)
}

fn profile() -> ProfileSpec {
    ProfileSpec::from_compile_time::<grafton_visca::profiles::PtzOpticsG2>().expect("profile")
}

#[cfg(feature = "blocking")]
mod blocking_tests {
    use super::*;
    use grafton_visca::{blocking::Session, config::SessionConfig};

    #[test]
    fn blocking_owner_admits_all_typed_raw_classes() {
        let fake = raw_camera();
        let session =
            Session::open(fake.blocking_wire(), &SessionConfig::new(profile())).expect("session");
        let camera = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>(
                grafton_visca::types::IndividualCameraId::CAMERA_1,
            )
            .expect("camera");
        let (plain, inquiry, targeted, applied) = raw_values();
        let iris_reset = grafton_visca::command::exposure::IrisReset;

        camera.execute(&plain).expect("plain");
        assert_eq!(camera.inquire(&inquiry).expect("inquiry"), 1);
        camera
            .submit::<grafton_visca::operation::Targeted, _>(&targeted)
            .expect("targeted")
            .settled()
            .expect("settled");
        camera
            .submit::<grafton_visca::operation::AppliedOnly, _>(&applied)
            .expect("applied-only")
            .applied()
            .expect("applied");
        camera
            .submit::<grafton_visca::operation::Targeted, _>(&iris_reset)
            .expect("typed iris targeted")
            .settled()
            .expect("typed iris settled");

        assert_eq!(
            fake.writes(),
            expected_writes(&plain, &inquiry, &targeted, &applied, &iris_reset)
        );
        session.shutdown().expect("shutdown");
    }
}

#[cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
mod async_tests {
    use super::*;
    use grafton_visca::{config::SessionConfig, r#async::Session, runtime::Executor};
    use grafton_visca_test_support::runtime_matrix;

    async fn owner_admits_all_typed_raw_classes<E: Executor>(executor: E) {
        let fake = raw_camera();
        let session = Session::open(fake.async_wire(), &SessionConfig::new(profile()), executor)
            .await
            .expect("session");
        let camera = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>(
                grafton_visca::types::IndividualCameraId::CAMERA_1,
            )
            .expect("camera");
        let (plain, inquiry, targeted, applied) = raw_values();
        let iris_reset = grafton_visca::command::exposure::IrisReset;

        camera.execute(&plain).await.expect("plain");
        assert_eq!(camera.inquire(&inquiry).await.expect("inquiry"), 1);
        camera
            .submit::<grafton_visca::operation::Targeted, _>(&targeted)
            .await
            .expect("targeted")
            .settled()
            .await
            .expect("settled");
        camera
            .submit::<grafton_visca::operation::AppliedOnly, _>(&applied)
            .await
            .expect("applied-only")
            .applied()
            .await
            .expect("applied");
        camera
            .submit::<grafton_visca::operation::Targeted, _>(&iris_reset)
            .await
            .expect("typed iris targeted")
            .settled()
            .await
            .expect("typed iris settled");

        assert_eq!(
            fake.writes(),
            expected_writes(&plain, &inquiry, &targeted, &applied, &iris_reset)
        );
        session.shutdown().expect("shutdown");
    }

    runtime_matrix!(owner_admits_all_typed_raw_classes);
}
