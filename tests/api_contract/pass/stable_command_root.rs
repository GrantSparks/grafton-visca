use grafton_visca::{
    command::{
        BoolConvention, CommandKind, FixedCommandBytes, InquiryKind, Nibbles, PanTiltDirection,
        Payload, PresetNumber, Response, ResponseParser, ViscaCommand, Zoom, ZoomPositionInquiry,
    },
    CameraId, Error,
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

    struct CustomCommand;

    impl ViscaCommand for CustomCommand {
        type Response = ();

        const MAX_SIZE: usize = 6;

        fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            let bytes = [camera_id.to_address_byte(), 0x01, 0x04, 0x00, 0x02, 0xFF];
            buffer[..bytes.len()].copy_from_slice(&bytes);
            Ok(bytes.len())
        }

        fn response_kind(&self) -> Option<InquiryKind> {
            None
        }
    }

    fn _assert_parser<T: ResponseParser>() {}

    let _encoded: FixedCommandBytes<6> = CustomCommand.to_fixed_bytes(CameraId::CAMERA_1).unwrap();
}
