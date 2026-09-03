use grafton_visca::{
    command::{ExposureMode, InquiryData},
    ViscaInquiry,
};

// These deliberately shadow names that unhygienic derive output used to rely
// on from the downstream prelude.
struct Result;
struct TryFrom;
enum Ok {}
enum Err {}

macro_rules! vec {
    ($($tokens:tt)*) => {
        compile_error!("derive output used the downstream vec! macro")
    };
}

macro_rules! format {
    ($($tokens:tt)*) => {
        compile_error!("derive output used the downstream format! macro")
    };
}

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(
    opcode = 0x39,
    response = ExposureMode,
    parser = Mode,
    value_type = ExposureMode
)]
struct ExposureModeProbe;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x47, response = ZoomPosition, parser = Position)]
struct ZoomPositionProbe;

#[derive(Debug, Copy, Clone, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power, parser = Bool)]
struct PowerProbe;

fn main() {
    assert!(matches!(
        ExposureModeProbe.parse_response(&[0x00]),
        ::core::result::Result::Ok(InquiryData::ExposureMode {
            mode: ExposureMode::Auto
        })
    ));
    assert!(ExposureModeProbe.parse_response(&[0x00, 0x00]).is_err());

    assert!(matches!(
        ZoomPositionProbe.parse_response(&[0x00, 0x01, 0x02, 0x03]),
        ::core::result::Result::Ok(InquiryData::ZoomPosition { position: 0x0123 })
    ));
    assert!(ZoomPositionProbe
        .parse_response(&[0x00, 0x01, 0x02, 0x03, 0x00])
        .is_err());
    assert!(ZoomPositionProbe
        .parse_response(&[0x10, 0x01, 0x02, 0x03])
        .is_err());

    assert!(PowerProbe.parse_response(&[0x02, 0x03]).is_err());

    let _ = (
        ::core::mem::size_of::<Result>(),
        ::core::mem::size_of::<TryFrom>(),
        ::core::mem::size_of::<Ok>(),
        ::core::mem::size_of::<Err>(),
    );
}
