use grafton_visca::{__macro_support::InquiryData, request::Inquiry, Error, ViscaInquiry};

#[derive(Debug, PartialEq, Eq)]
struct LocalLevel(u8);
impl LocalLevel {
    fn new(value: grafton_visca::types::ZoomPosition) -> Result<Self, Error> {
        let value = u8::try_from(value.value()).map_err(|_| Error::invalid_response("local level fitting u8", Vec::new()))?;
        Ok(Self(value))
    }
}

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x47, response = ZoomPosition, typed_response = LocalLevel, typed_field = position, typed_constructor = new)]
struct LocalInquiry;

fn main() {
    assert_eq!(
        LocalInquiry.decoder().decode(&[0, 0, 0, 5]).unwrap(),
        LocalLevel(5)
    );
    assert!(matches!(
        LocalInquiry.decoder().decode(&[0, 1, 0, 5]),
        Err(Error::InvalidResponse { .. })
    ));
    assert!(matches!(
        LocalInquiry.parse_response(&[0, 0, 0, 5]),
        Ok(InquiryData::ZoomPosition { position }) if position.value() == 5
    ));
}
