use grafton_visca::{
    command::{
        CommandKind, FixedCommandBytes, ImageFreeze, InquiryKind, Response, ResponseParser,
        ViscaCommand,
    },
    timeout::CommandCategory,
    CameraId, Error,
};

struct CustomCommand;
struct CustomPowerInquiry;

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

    fn response_kind(&self) -> Option<InquiryKind> {
        None
    }
}

impl ViscaCommand for CustomPowerInquiry {
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

    fn response_kind(&self) -> Option<InquiryKind> {
        Some(InquiryKind::Power)
    }
}

impl ResponseParser for CustomPowerInquiry {
    type Response = bool;

    fn from_response(resp: Response) -> Result<Self::Response, Error> {
        match resp {
            Response::Inquiry(grafton_visca::command::InquiryData::Power { on }) => Ok(on),
            Response::Error(error) => Err(error),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

fn main() {
    let command = CustomCommand;
    let encoded: FixedCommandBytes<2> = command.to_fixed_bytes::<2>(CameraId::CAMERA_1).unwrap();
    assert_eq!(encoded.as_slice(), &[0x81, 0xFF]);
    assert_eq!(command.command_kind(), CommandKind::Command);
    assert_eq!(CustomPowerInquiry.command_kind(), CommandKind::Inquiry);
    let _raw_freeze_command = ImageFreeze::on();
}
