use grafton_visca::{__macro_support::WireEncode, visca_command, CameraId, Error};

visca_command! {
    pub struct ExternalPlain;
    bytes = [0x01, 0x04, 0x00, 0x02];
}
visca_command! {
    pub struct ExternalParameter { value: u8 };
    prefix = [0x01, 0x04, 0x00];
    param = *value;
    max_param_size = 1;
}

fn main() {
    let mut bytes = [0; 6];
    assert_eq!(ExternalPlain.write_into(CameraId::CAMERA_1, &mut bytes).unwrap(), 6);
    let parameter = ExternalParameter { value: 2 };
    assert_eq!(parameter.write_into(CameraId::CAMERA_1, &mut bytes).unwrap(), 6);
    assert!(matches!(parameter.write_into(CameraId::CAMERA_1, &mut []), Err(Error::BufferTooSmall { required: 6, actual: 0, .. })));
}
