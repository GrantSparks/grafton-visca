use grafton_visca::{
    request::ControlClass, request::Inquiry, request::InquiryClass, request::PlainClass,
    request::PlainCommand, request::Request, request::ResponseDecoder, request::CommandRetryClass,
    profiles::CommandTimeoutClass, types::CameraId, wire::Response, wire::ResponseParser,
    wire::VISCA_TERMINATOR, Error,
};

struct CustomCommand;
struct VendorStatusInquiry;

impl Request for CustomCommand {
    type Class = PlainClass;

    const MAX_SIZE: usize = 2;
    fn policy(&self) -> grafton_visca::request::CommandPolicy {
        grafton_visca::request::CommandPolicy::new(grafton_visca::profiles::CommandTimeoutClass::Quick, grafton_visca::request::CommandRetryClass::Never, grafton_visca::request::SubmissionClass::Normal)
    }

    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::buffer_too_small(Self::MAX_SIZE, buffer.len()));
        }

        buffer[0] = 0x80 | camera_id.value();
        buffer[1] = 0xFF;
        Ok(Self::MAX_SIZE)
    }
}

impl Request for VendorStatusInquiry {
    type Class = InquiryClass;

    const MAX_SIZE: usize = 5;
    fn policy(&self) -> grafton_visca::request::InquiryPolicy {
        grafton_visca::request::InquiryPolicy::new(grafton_visca::request::InquiryRetryClass::Inquiry, grafton_visca::request::SubmissionClass::Normal)
    }

    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        let bytes = [
            (0x80 | camera_id.value()),
            0x09,
            0x04,
            0x00,
            VISCA_TERMINATOR,
        ];
        if buffer.len() < bytes.len() {
            return Err(Error::buffer_too_small(bytes.len(), buffer.len()));
        }
        buffer[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }
}

impl Inquiry for VendorStatusInquiry {
    type Response = u8;

    fn decoder(&self) -> ResponseDecoder<Self::Response> {
        ResponseDecoder::from_fn(|payload| {
            payload
                .first()
                .copied()
                .ok_or(Error::UnexpectedResponseType)
        })
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
    fn assert_plain<T: PlainCommand>() {}
    fn assert_inquiry<T: Inquiry>() {}

    assert_plain::<CustomCommand>();
    assert_inquiry::<VendorStatusInquiry>();

    let command = CustomCommand;
    let mut command_buffer = [0; CustomCommand::MAX_SIZE];
    let command_len = command
        .write_into(CameraId::CAMERA_1, &mut command_buffer)
        .unwrap();
    assert_eq!(&command_buffer[..command_len], &[0x81, VISCA_TERMINATOR]);

    let mut inquiry_buffer = [0; VendorStatusInquiry::MAX_SIZE];
    let inquiry_len = VendorStatusInquiry
        .write_into(CameraId::CAMERA_1, &mut inquiry_buffer)
        .unwrap();
    assert_eq!(inquiry_len, VendorStatusInquiry::MAX_SIZE);
    let payload = <VendorStatusInquiry as Inquiry>::decoder(&VendorStatusInquiry)
        .decode(&[0x7f])
        .unwrap();
    assert_eq!(payload, 0x7f);

    let response =
        Response::RawInquiry(grafton_visca::wire::RawInquiryPayload::from_slice(&[0x7f]));
    assert_eq!(VendorStatusInquiry::from_response(response).unwrap(), 0x7f);
    let _raw_freeze_command = grafton_visca::command::image::SetFreeze::new(true);
}
