use grafton_visca::{__macro_support::WireEncode, visca_command, CameraId, Error};

const EXTERNAL_BODY: [u8; 4] = [0x01, 0x04, 0x00, 0x03];

visca_command! {
    pub struct ExternalPlain;
    bytes = [0x01, 0x04, 0x00, 0x02];
}
visca_command! {
    pub struct ExternalConstBody;
    bytes = EXTERNAL_BODY;
}
visca_command! {
    pub struct ExternalParameter { value: u8 };
    prefix = [0x01, 0x04, 0x00];
    param = *value;
    max_param_size = 1;
}

// A unit command gets a `const fn new()` and `Default` from the macro.
const PLAIN: ExternalPlain = ExternalPlain::new();

fn main() {
    let mut bytes = [0; 6];
    assert_eq!(PLAIN.write_into(CameraId::CAMERA_1, &mut bytes).unwrap(), 6);
    assert_eq!(bytes, [0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]);
    assert_eq!(ExternalPlain::default().write_into(CameraId::CAMERA_2, &mut bytes).unwrap(), 6);
    assert_eq!(bytes, [0x82, 0x01, 0x04, 0x00, 0x02, 0xFF]);
    assert_eq!(ExternalConstBody::new().write_into(CameraId::CAMERA_1, &mut bytes).unwrap(), 6);
    assert_eq!(bytes, [0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]);
    let parameter = ExternalParameter { value: 2 };
    assert_eq!(parameter.write_into(CameraId::CAMERA_1, &mut bytes).unwrap(), 6);
    assert!(matches!(parameter.write_into(CameraId::CAMERA_1, &mut []), Err(Error::BufferTooSmall { required: 6, actual: 0, .. })));
}
