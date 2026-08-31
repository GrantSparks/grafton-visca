#[path = "common/compile_fail.rs"]
mod compile_fail;

use grafton_visca::{
    command::{InquiryData, VISCA_TERMINATOR},
    CameraId, Request, ViscaInquiry,
};

/// Mirrors the documented downstream `parser = PanTilt` use case.
#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x12, subcode = 0x06, response = PanTiltPosition, parser = PanTilt)]
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
    let features = compile_fail::active_grafton_visca_features();
    compile_fail::assert_compile_pass_fixture_paths(
        &["tests/api_contract/pass/derive_macro_pan_tilt_standard.rs"],
        &features,
    );
}
