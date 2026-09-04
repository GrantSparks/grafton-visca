use grafton_visca::command::{
    BoolConvention, CommandKind, InquiryKind, Nibbles, PanTiltDirection, Payload, PresetNumber,
    RawInquiryPayload, Response, ResponseParser, Zoom, ZoomPositionInquiry, VISCA_TERMINATOR,
};

fn main() {
    let _ = core::any::TypeId::of::<Response>();
    let _ = core::any::TypeId::of::<Zoom>();
    let _ = core::any::TypeId::of::<ZoomPositionInquiry>();
    let _ = core::any::TypeId::of::<PanTiltDirection>();
    let _ = core::any::TypeId::of::<PresetNumber>();
    let _ = core::any::TypeId::of::<Nibbles<4>>();
    let _ = BoolConvention::OnIs02;
    let _ = Payload::new(&[0x02]);
    let _ = InquiryKind::Power;
    let _ = CommandKind::Command;
    let _ = CommandKind::Inquiry;
    let _ = VISCA_TERMINATOR;

    fn _assert_parser<T: ResponseParser>() {}

    let _ = core::any::TypeId::of::<RawInquiryPayload>();
}
