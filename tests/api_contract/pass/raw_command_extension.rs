use grafton_visca::{
    command::{
        CommandBehavior, CommandKind, FixedCommandBytes, ImageFreeze, InquiryResponseSpec,
        Response, ResponseParser, ViscaCommand,
    },
    timeout::CommandCategory,
    CameraId, Error,
};

struct CustomCommand;
struct VendorStatusInquiry;

impl ViscaCommand for CustomCommand {
    const MAX_SIZE: usize = 2;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len(),
            });
        }

        buffer[0] = camera_id.to_address_byte();
        buffer[1] = 0xFF;
        Ok(Self::MAX_SIZE)
    }
}

impl ViscaCommand for VendorStatusInquiry {
    const MAX_SIZE: usize = 5;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        let bytes = [
            camera_id.to_address_byte(),
            0x09,
            0x04,
            0x00,
            grafton_visca::command::VISCA_TERMINATOR,
        ];
        buffer[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }

    fn behavior(&self) -> CommandBehavior {
        CommandBehavior::Inquiry(InquiryResponseSpec::Raw)
    }
}

impl ResponseParser for VendorStatusInquiry {
    type Response = u8;

    fn from_response(resp: Response) -> Result<Self::Response, Error> {
        match resp {
            Response::RawInquiry(payload) => payload
                .as_slice()
                .first()
                .copied()
                .ok_or(Error::UnexpectedResponseType),
            Response::Error(error) => Err(error),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

fn main() {
    let command = CustomCommand;
    let encoded: FixedCommandBytes<2> = command.to_fixed_bytes::<2>(CameraId::CAMERA_1).unwrap();
    assert_eq!(encoded.as_slice(), &[0x81, 0xFF]);
    assert_eq!(command.behavior().command_kind(), CommandKind::Command);
    assert_eq!(
        VendorStatusInquiry.behavior(),
        CommandBehavior::Inquiry(InquiryResponseSpec::Raw)
    );
    assert_eq!(
        VendorStatusInquiry.behavior().command_kind(),
        CommandKind::Inquiry
    );
    let _raw_freeze_command = ImageFreeze::on();
}
