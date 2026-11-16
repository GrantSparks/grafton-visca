use grafton_visca::{
    command::zoom::Stop, types::PanTiltDirection, types::PresetNumber, wire::BoolConvention,
    wire::CommandKind, wire::Nibbles, wire::Payload, wire::RawInquiryPayload, wire::Response,
    wire::ResponseParser, wire::VISCA_TERMINATOR,
};

fn main() {
    let _ = core::any::TypeId::of::<Response>();
    let _ = core::any::TypeId::of::<Stop>();
    let _ = core::any::TypeId::of::<grafton_visca::command::zoom::PositionInquiry>();
    let _ = core::any::TypeId::of::<PanTiltDirection>();
    let _ = core::any::TypeId::of::<PresetNumber>();
    let _ = core::any::TypeId::of::<Nibbles<4>>();
    let _ = BoolConvention::OnIs02;
    let _ = Payload::new(&[0x02]);
    let _ = CommandKind::Command;
    let _ = CommandKind::Inquiry;
    let _ = VISCA_TERMINATOR;

    fn _assert_parser<T: ResponseParser>() {}

    let _ = core::any::TypeId::of::<RawInquiryPayload>();
}
