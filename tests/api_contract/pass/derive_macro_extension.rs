use grafton_visca::{
    command::{InquiryKind, VISCA_TERMINATOR},
    CameraId, Inquiry, InquiryRoute, Request, ViscaInquiry,
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
    let len = <PowerProbeInquiry as Request>::write_into(&inquiry, CameraId::CAMERA_1, &mut buffer)
        .unwrap();
    assert_eq!(&buffer[..len], &[0x81, 0x09, 0x04, 0x00, VISCA_TERMINATOR]);
    assert_eq!(
        inquiry.route(),
        InquiryRoute::custom(InquiryKind::Power as u16 + 1)
    );

    let raw = VendorRawInquiry;
    let mut raw_buffer = [0; VendorRawInquiry::MAX_SIZE];
    let raw_len =
        <VendorRawInquiry as Request>::write_into(&raw, CameraId::CAMERA_1, &mut raw_buffer)
            .unwrap();
    assert_eq!(raw_len, VendorRawInquiry::MAX_SIZE);
    assert_eq!(raw.route(), InquiryRoute::RAW);
}
