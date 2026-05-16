use grafton_visca::{
    command::{BoolConvention, Nibbles, Payload, Response},
    CachedFlipState, CameraId, PanTiltLimits, StateCache, ViscaSocket,
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
}
