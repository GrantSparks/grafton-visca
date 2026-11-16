//! Regression coverage for the source-backed exposure/iris profile boundary.

#![allow(clippy::expect_used)]

use core::marker::PhantomData;

use grafton_visca::{
    capabilities::{HasExposureMode, HasIris, TypedSupportSurface},
    profiles::{
        CompileTimeProfile, GenericVisca, NearusBrc300, PtzOptics30x, PtzOpticsG2, PtzOpticsG3,
        SonyBrc300, SonyBrcH900, SonyEviH100, SonyFr7,
    },
    request::Request,
    types::{CameraId, ExposureMode, IrisLevel},
};

fn wire<R: Request + ?Sized>(request: &R) -> Vec<u8> {
    let mut buffer = vec![0_u8; R::MAX_SIZE];
    let written = request
        .write_into(CameraId::CAMERA_1, &mut buffer)
        .expect("source-backed exposure request must encode");
    buffer.truncate(written);
    buffer
}

fn source_backed_requests<P>(
    _: PhantomData<P>,
) -> (
    grafton_visca::command::exposure::SetMode,
    grafton_visca::command::exposure::SetIris,
)
where
    P: CompileTimeProfile + HasExposureMode + HasIris,
{
    (
        grafton_visca::command::exposure::SetMode::new(ExposureMode::Auto),
        grafton_visca::command::exposure::SetIris::new(
            IrisLevel::new(0x0C).expect("valid iris level"),
        ),
    )
}

#[test]
fn brc_h900_and_brc_300_typed_requests_encode_the_documented_standard_frames() {
    for (exposure, iris) in [
        source_backed_requests::<SonyBrcH900>(PhantomData),
        source_backed_requests::<SonyBrc300>(PhantomData),
    ] {
        assert_eq!(wire(&exposure), [0x81, 0x01, 0x04, 0x39, 0x00, 0xFF]);
        assert_eq!(
            wire(&iris),
            [0x81, 0x01, 0x04, 0x4B, 0x00, 0x00, 0x00, 0x0C, 0xFF]
        );
    }

    assert_eq!(
        grafton_visca::profiles::ProfileSpec::from_compile_time::<SonyBrcH900>()
            .expect("validated profile")
            .capabilities()
            .clone()
            .exposure
            .as_ref()
            .expect("exposure domain")
            .modes,
        [
            ExposureMode::Auto,
            ExposureMode::Manual,
            ExposureMode::Shutter,
            ExposureMode::Iris,
        ],
        "R11 does not list Bright mode for BRC-H900"
    );
    assert!(
        grafton_visca::profiles::ProfileSpec::from_compile_time::<SonyBrc300>()
            .expect("validated profile")
            .capabilities()
            .clone()
            .exposure
            .as_ref()
            .expect("exposure domain")
            .modes
            .contains(&ExposureMode::Bright),
        "R12 lists Bright mode for BRC-300"
    );
}

fn assert_shared_exposure_inventory<P>()
where
    P: CompileTimeProfile + HasExposureMode + HasIris,
{
    let capabilities = grafton_visca::profiles::ProfileSpec::from_compile_time::<P>()
        .expect("validated profile")
        .capabilities()
        .clone();
    assert!(!capabilities
        .exposure
        .as_ref()
        .expect("exposure domain")
        .modes
        .is_empty());
    assert!(capabilities.supports(TypedSupportSurface::ExposureMode));
    assert!(capabilities.supports(TypedSupportSurface::Iris));
}

#[test]
fn every_builtin_exposure_inventory_matches_its_typed_marker_boundary() {
    assert_shared_exposure_inventory::<PtzOpticsG2>();
    assert_shared_exposure_inventory::<PtzOpticsG3>();
    assert_shared_exposure_inventory::<PtzOptics30x>();
    assert_shared_exposure_inventory::<SonyBrcH900>();
    assert_shared_exposure_inventory::<SonyEviH100>();
    assert_shared_exposure_inventory::<SonyBrc300>();
    assert_shared_exposure_inventory::<NearusBrc300>();
    assert_shared_exposure_inventory::<GenericVisca>();

    let fr7 = grafton_visca::profiles::ProfileSpec::from_compile_time::<SonyFr7>()
        .expect("validated profile")
        .capabilities()
        .clone();
    assert!(fr7
        .exposure
        .as_ref()
        .expect("exposure domain")
        .modes
        .is_empty());
    assert!(!fr7.supports(TypedSupportSurface::ExposureMode));
    assert!(!fr7.supports(TypedSupportSurface::Iris));
}
