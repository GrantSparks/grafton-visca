use grafton_visca_test_support::{active_grafton_visca_features, compile_fail};

use grafton_visca::{
    __macro_support::InquiryData, request::Request, types::CameraId, wire::VISCA_TERMINATOR,
    ViscaInquiry,
};

/// Mirrors canonical downstream pan/tilt inquiry decoding.
#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x12, subcode = 0x06, response = PanTiltPosition)]
struct StandardPanTiltProbeInquiry;

#[test]
fn downstream_pantilt_derive_keeps_standard_framing_and_widens_signed_endpoints() {
    let inquiry = StandardPanTiltProbeInquiry;
    let mut request = [0; StandardPanTiltProbeInquiry::MAX_SIZE];
    let request_len = inquiry
        .write_into(CameraId::CAMERA_1, &mut request)
        .expect("standard PanTilt inquiry should encode");
    assert_eq!(
        &request[..request_len],
        &[0x81, 0x09, 0x06, 0x12, VISCA_TERMINATOR]
    );

    match inquiry
        .parse_response(&[0x08, 0x00, 0x00, 0x00, 0x07, 0x0F, 0x0F, 0x0F])
        .expect("standard 4+4 PanTilt reply should decode")
    {
        InquiryData::PanTiltPosition { pan, tilt } => {
            let _: i32 = pan;
            let _: i32 = tilt;
            assert_eq!((pan, tilt), (-32_768, 32_767));
        }
        _ => panic!("PanTilt parser must return PanTiltPosition"),
    }

    assert!(
        inquiry.parse_response(&[0; 9]).is_err(),
        "the generic parser accepts only the standard 4+4 reply"
    );
}

#[test]
fn documented_downstream_pantilt_derive_compiles() {
    let features = active_grafton_visca_features!();
    compile_fail::assert_compile_pass_fixture_paths(
        &["tests/api_contract/pass/derive_macro_pan_tilt_standard.rs"],
        &features,
    );
}

#[cfg(feature = "blocking")]
#[test]
fn derived_pantilt_inquiry_requires_profile_framing_before_io() {
    use grafton_visca::{
        blocking::Session,
        capabilities::Capabilities,
        config::SessionConfig,
        profiles::{AnyProfile, ProfileEnvelope, ProfileSpec, PtzOpticsG2},
        types::{AffectedAxes, IndividualCameraId},
        Error,
    };
    use grafton_visca_test_support::fake_camera::FakeCamera;

    let baseline = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile");
    let mut capabilities = Capabilities::runtime_baseline("Zoom-only camera", 1).expect("target");
    capabilities.zoom.clone_from(&baseline.capabilities().zoom);
    let profile = ProfileSpec::builder(capabilities)
        .transports(baseline.transports())
        .envelope(ProfileEnvelope::RawVisca)
        .timing(baseline.timing())
        .maximum_command_sockets(baseline.maximum_command_sockets())
        .supports_operation_complete(false)
        .supports_command_cancel(false)
        .position_inquiries(AffectedAxes::ZOOM)
        .inquiries(true)
        .build()
        .expect("zoom-only profile");

    let fake = FakeCamera::silent();
    let session =
        Session::open(fake.blocking_wire(), &SessionConfig::new(profile)).expect("session");
    let camera = session
        .camera::<AnyProfile>(IndividualCameraId::CAMERA_1)
        .expect("camera");
    assert!(matches!(
        camera.inquire(&StandardPanTiltProbeInquiry),
        Err(Error::FeatureNotSupported { .. })
    ));
    assert!(fake.writes().is_empty());
    session.shutdown().expect("owner shutdown");
}
