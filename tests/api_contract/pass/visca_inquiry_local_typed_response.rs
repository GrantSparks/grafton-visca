use grafton_visca::{command::InquiryData, Error, Inquiry, ViscaInquiry};

#[derive(Debug, PartialEq, Eq)]
struct LocalLevel(u8);
impl LocalLevel {
    fn new(value: u8) -> Result<Self, Error> { Ok(Self(value)) }
}

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x47, response = ZoomPosition, typed_response = LocalLevel, typed_field = position, typed_constructor = new_u8)]
struct LocalInquiry;

fn main() {
    assert_eq!(LocalInquiry.decoder().decode(&[0, 0, 0, 5]).unwrap(), LocalLevel(5));
    assert!(matches!(LocalInquiry.decoder().decode(&[0, 1, 0, 5]), Err(Error::InvalidResponse { .. })));
    assert!(matches!(LocalInquiry.parse_response(&[0, 0, 0, 5]), Ok(InquiryData::ZoomPosition { position: 5 })));
}
