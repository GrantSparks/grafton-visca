use grafton_visca::{
    command::{ViscaCommand, VISCA_TERMINATOR},
    CameraId, ViscaInquiry,
};

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x00, response = "Power", parser = "bool")]
struct PowerProbeInquiry;

fn main() {
    let inquiry = PowerProbeInquiry;
    let mut buffer = [0; PowerProbeInquiry::MAX_SIZE];
    let len = inquiry.write_into(CameraId::CAMERA_1, &mut buffer).unwrap();
    assert_eq!(&buffer[..len], &[0x81, 0x09, 0x04, 0x00, VISCA_TERMINATOR]);
    assert_eq!(
        inquiry.response_kind(),
        Some(grafton_visca::command::InquiryKind::Power)
    );
}
