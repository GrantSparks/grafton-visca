use grafton_visca::{
    command::{InquiryData, VISCA_TERMINATOR},
    CameraId, Request, ViscaInquiry,
};

/// A downstream derive uses only the standard VISCA inquiry and reply forms.
#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x12, subcode = 0x06, response = PanTiltPosition, parser = PanTilt)]
struct StandardPanTiltProbeInquiry;

fn main() {
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
        _ => unreachable!("PanTilt parser must return PanTiltPosition"),
    }

    assert!(
        inquiry.parse_response(&[0; 9]).is_err(),
        "the generic parser accepts only the standard 4+4 reply"
    );
}
