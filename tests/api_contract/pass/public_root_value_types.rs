use grafton_visca::{
    command::{BoolConvention, Nibbles, Payload, Response},
    CachedFlipState, CameraId, PanTiltLimits, StateCache, UnitInterval, ViscaSocket,
};

fn main() {
    let camera_id = CameraId::CAMERA_1;
    assert_eq!(camera_id.to_address_byte(), 0x81);

    let _socket = ViscaSocket::S1;
    let _cache = StateCache::new();
    let _limits = PanTiltLimits::new();
    let _flip = CachedFlipState {
        horizontal: false,
        vertical: false,
    };
    let _response = Response::Completion { socket: None };
    let payload = Payload::new(&[0x02]);
    let _ = payload
        .parse_bool("contract", BoolConvention::OnIs02)
        .unwrap();
    let _ = Nibbles::<4>::try_from(Payload::new(&[0, 1, 2, 3])).unwrap();
    let root_value = UnitInterval::new(0.5).unwrap();
    let units_value: grafton_visca::units::UnitInterval = root_value;
    assert_eq!(units_value.value(), 0.5);

    #[cfg(feature = "mode-async")]
    {
        let prelude_value: grafton_visca::prelude::r#async::UnitInterval = root_value;
        assert_eq!(prelude_value.value(), 0.5);
    }

    #[cfg(not(feature = "mode-async"))]
    {
        let prelude_value: grafton_visca::prelude::blocking::UnitInterval = root_value;
        assert_eq!(prelude_value.value(), 0.5);
    }
}
