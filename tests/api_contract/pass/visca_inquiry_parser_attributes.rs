use grafton_visca::{
    command::{ExposureMode, InquiryData},
    Error, Inquiry, ViscaInquiry,
};

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(
    opcode = 0x39, response = ExposureMode, typed_response = ExposureMode, typed_field = mode
)]
struct ModeInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(
    opcode = 0x4B, response = Iris,
    typed_response = u8, typed_field = position
)]
struct NibbleInquiry;

fn custom(payload: &[u8]) -> Result<InquiryData, Error> {
    Ok(InquiryData::Digital {
        on: payload == [0xA5],
    })
}

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(
    opcode = 0x00, response = Power, parser = Custom,
    parse_with = custom, data_variant = Digital,
    typed_response = bool, typed_field = on
)]
struct CustomInquiry;

fn main() {
    assert_eq!(
        ModeInquiry.decoder().decode(&[0x00]).unwrap(),
        ExposureMode::Auto
    );
    assert_eq!(NibbleInquiry.decoder().decode(&[0x00, 0x00, 0x00, 0x05]).unwrap(), 5);
    assert!(CustomInquiry.decoder().decode(&[0xA5]).unwrap());
}
