use grafton_visca::{
    types::CameraId, types::ImageFlipMode, types::UnitInterval, types::ViscaSocket,
    types::ZoomDomain, wire::BoolConvention, wire::Nibbles, wire::Payload, wire::Response,
};

fn main() {
    let _: Option<grafton_visca::error::Error> = None;
    let camera_id = CameraId::CAMERA_1;
    assert_eq!((0x80 | camera_id.value()), 0x81);

    let _socket = ViscaSocket::S1;
    let _flip = ImageFlipMode::Off;
    let _position =
        grafton_visca::types::PanTiltPosition::from_degrees(0.0, 0.0).expect("finite position");
    let _response = Response::completion(None);
    let payload = Payload::new(&[0x02]);
    let _ = payload
        .parse_bool("contract", BoolConvention::OnIs02)
        .unwrap();
    let _ = Nibbles::<4>::try_from(Payload::new(&[0, 1, 2, 3])).unwrap();
    let root_value = UnitInterval::new(0.5).unwrap();
    let units_value: grafton_visca::types::UnitInterval = root_value;
    assert_eq!(units_value.value(), 0.5);
    let _domain = ZoomDomain::Optical;

    #[cfg(feature = "async")]
    {
        let prelude_value: grafton_visca::prelude::r#async::UnitInterval = root_value;
        assert_eq!(prelude_value.value(), 0.5);
    }

    #[cfg(feature = "blocking")]
    {
        let prelude_value: grafton_visca::prelude::blocking::UnitInterval = root_value;
        assert_eq!(prelude_value.value(), 0.5);
    }
}
