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

#[path = "common/fake_camera.rs"]
mod fake_camera;
#[cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
#[macro_use]
#[path = "common/matrix.rs"]
mod matrix;

use grafton_visca::{
    raw, AffectedAxes, ControlClass, Error, InquiryRoute, ProfileSpec, ResponseDecoder, RetryClass,
    TimeoutClass,
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
    plain: &raw::Plain,
    inquiry: &raw::Inquiry<u8>,
    targeted: &raw::Targeted,
    applied: &raw::AppliedOnly,
    iris_reset: &grafton_visca::request::builtin::IrisReset,
) -> Vec<Vec<u8>> {
    vec![
        plain.bytes().to_vec(),
        inquiry.bytes().to_vec(),
        targeted.bytes().to_vec(),
        applied.bytes().to_vec(),
        {
            let mut bytes = [0_u8; 8];
            let length = grafton_visca::Request::write_into(
                iris_reset,
                grafton_visca::CameraId::CAMERA_1,
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
        .ok_or_else(|| Error::InvalidRequest("empty raw inquiry payload".into()))
}

fn raw_values() -> (
    raw::Plain,
    raw::Inquiry<u8>,
    raw::Targeted,
    raw::AppliedOnly,
) {
    let plain = raw::Plain::new(
        [0x81, 0x01, 0x01, 0xff],
        TimeoutClass::Quick,
        RetryClass::Never,
        ControlClass::Normal,
    )
    .expect("plain frame");
    let inquiry = raw::Inquiry::new(
        [0x81, 0x09, 0x01, 0xff],
        InquiryRoute::RAW,
        ResponseDecoder::from_fn(decode_first),
        TimeoutClass::Inquiry,
        RetryClass::Inquiry,
        ControlClass::Normal,
    )
    .expect("inquiry frame");
    let targeted = raw::Targeted::new(
        [0x81, 0x01, 0x06, 0xff],
        AffectedAxes::PAN_TILT,
        TimeoutClass::Movement,
        RetryClass::Movement,
        ControlClass::User,
    )
    .expect("targeted frame");
    // A raw applied-only operation classifies itself `User`, the highest lane a
    // raw caller may select; the urgent safety lane is owner-only (#679).
    let applied = raw::AppliedOnly::new(
        [0x81, 0x01, 0x07, 0xff],
        AffectedAxes::ZOOM,
        TimeoutClass::Quick,
        RetryClass::Never,
        ControlClass::User,
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
    use grafton_visca::blocking::{Session, SessionConfig};

    #[test]
    fn blocking_owner_admits_all_typed_raw_classes() {
        let fake = raw_camera();
        let session =
            Session::open(fake.blocking_wire(), SessionConfig::new(profile())).expect("session");
        let camera = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .expect("camera");
        let (plain, inquiry, targeted, applied) = raw_values();
        let iris_reset = grafton_visca::request::builtin::IrisReset::new();

        camera.execute(&plain).expect("plain");
        assert_eq!(camera.inquire(&inquiry).expect("inquiry"), 1);
        camera
            .submit::<grafton_visca::completion::Targeted, _>(&targeted)
            .expect("targeted")
            .settled()
            .expect("settled");
        camera
            .submit::<grafton_visca::completion::AppliedOnly, _>(&applied)
            .expect("applied-only")
            .applied()
            .expect("applied");
        camera
            .submit::<grafton_visca::completion::Targeted, _>(&iris_reset)
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
    use grafton_visca::{Executor, Session, SessionConfig};

    async fn owner_admits_all_typed_raw_classes<E: Executor>(executor: E) {
        let fake = raw_camera();
        let session = Session::open(fake.async_wire(), SessionConfig::new(profile()), executor)
            .await
            .expect("session");
        let camera = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .expect("camera");
        let (plain, inquiry, targeted, applied) = raw_values();
        let iris_reset = grafton_visca::request::builtin::IrisReset::new();

        camera.execute(&plain).await.expect("plain");
        assert_eq!(camera.inquire(&inquiry).await.expect("inquiry"), 1);
        camera
            .submit::<grafton_visca::completion::Targeted, _>(&targeted)
            .await
            .expect("targeted")
            .settled()
            .await
            .expect("settled");
        camera
            .submit::<grafton_visca::completion::AppliedOnly, _>(&applied)
            .await
            .expect("applied-only")
            .applied()
            .await
            .expect("applied");
        camera
            .submit::<grafton_visca::completion::Targeted, _>(&iris_reset)
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
