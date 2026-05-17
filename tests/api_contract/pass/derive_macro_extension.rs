use grafton_visca::{
    command::{CommandBehavior, InquiryKind, InquiryResponseSpec, ViscaCommand, VISCA_TERMINATOR},
    CameraId, ViscaInquiry,
};

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power, parser = Bool)]
struct PowerProbeInquiry;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x7E, response = Raw)]
struct VendorRawInquiry;

fn main() {
    let inquiry = PowerProbeInquiry;
    let mut buffer = [0; PowerProbeInquiry::MAX_SIZE];
    let len = inquiry.write_into(CameraId::CAMERA_1, &mut buffer).unwrap();
    assert_eq!(&buffer[..len], &[0x81, 0x09, 0x04, 0x00, VISCA_TERMINATOR]);
    assert_eq!(
        inquiry.behavior(),
        CommandBehavior::Inquiry(InquiryResponseSpec::Builtin(InquiryKind::Power))
    );
    assert_eq!(
        VendorRawInquiry.behavior(),
        CommandBehavior::Inquiry(InquiryResponseSpec::Raw)
    );
}
