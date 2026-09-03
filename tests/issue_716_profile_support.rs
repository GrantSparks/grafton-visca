//! Regression coverage for the source-backed exposure/iris profile boundary.

#![allow(clippy::expect_used)]

use core::marker::PhantomData;

use grafton_visca::{
    capabilities::{Capabilities, HasExposureMode, HasIrisControl, TypedSupportSurface},
    command::{ExposureCommand, ExposureMode},
    profiles::{
        GenericVisca, NearusBRC300, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300,
        SonyBRCH900, SonyEVIH100, SonyFR7,
    },
    request::builtin::IrisDirect,
    types::IrisLevel,
    CameraId, CompileTimeProfile, Request,
};

fn wire<R: Request + ?Sized>(request: &R) -> Vec<u8> {
    let mut buffer = vec![0_u8; R::MAX_SIZE];
    let written = request
        .write_into(CameraId::CAMERA_1, &mut buffer)
        .expect("source-backed exposure request must encode");
    buffer.truncate(written);
    buffer
}

fn source_backed_requests<P>(_: PhantomData<P>) -> (ExposureCommand, IrisDirect)
where
    P: CompileTimeProfile + HasExposureMode + HasIrisControl,
{
    (
        ExposureCommand::new(ExposureMode::Auto),
        IrisDirect::new(IrisLevel::new(0x0C).expect("valid iris level")),
    )
}

#[test]
fn brc_h900_and_brc_300_typed_requests_encode_the_documented_standard_frames() {
    for (exposure, iris) in [
        source_backed_requests::<SonyBRCH900>(PhantomData),
        source_backed_requests::<SonyBRC300>(PhantomData),
    ] {
        assert_eq!(wire(&exposure), [0x81, 0x01, 0x04, 0x39, 0x00, 0xFF]);
        assert_eq!(
            wire(&iris),
            [0x81, 0x01, 0x04, 0x4B, 0x00, 0x00, 0x00, 0x0C, 0xFF]
        );
    }

    assert_eq!(
        Capabilities::from_profile::<SonyBRCH900>().exposure_modes,
        [
            ExposureMode::Auto,
            ExposureMode::Manual,
            ExposureMode::Shutter,
            ExposureMode::Iris,
        ],
        "R11 does not list Bright mode for BRC-H900"
    );
    assert!(
        Capabilities::from_profile::<SonyBRC300>()
            .exposure_modes
            .contains(&ExposureMode::Bright),
        "R12 lists Bright mode for BRC-300"
    );
}

fn assert_shared_exposure_inventory<P>()
where
    P: CompileTimeProfile + HasExposureMode + HasIrisControl,
{
    let capabilities = Capabilities::from_profile::<P>();
    assert!(!capabilities.exposure_modes.is_empty());
    assert!(capabilities.supports_typed(TypedSupportSurface::ExposureMode));
    assert!(capabilities.supports_typed(TypedSupportSurface::IrisControl));
}

#[test]
fn every_builtin_exposure_inventory_matches_its_typed_marker_boundary() {
    assert_shared_exposure_inventory::<PtzOpticsG2>();
    assert_shared_exposure_inventory::<PtzOpticsG3>();
    assert_shared_exposure_inventory::<PtzOptics30X>();
    assert_shared_exposure_inventory::<SonyBRCH900>();
    assert_shared_exposure_inventory::<SonyEVIH100>();
    assert_shared_exposure_inventory::<SonyBRC300>();
    assert_shared_exposure_inventory::<NearusBRC300>();
    assert_shared_exposure_inventory::<GenericVisca>();

    let fr7 = Capabilities::from_profile::<SonyFR7>();
    assert!(fr7.exposure_modes.is_empty());
    assert!(!fr7.supports_typed(TypedSupportSurface::ExposureMode));
    assert!(!fr7.supports_typed(TypedSupportSurface::IrisControl));
}
