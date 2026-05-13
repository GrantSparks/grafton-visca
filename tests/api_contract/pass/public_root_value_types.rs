use grafton_visca::{
    CachedFlipState, CameraId, CameraVariant, PanTiltLimits, StateCache, ViscaSocket,
};

fn main() {
    let camera_id = CameraId::CAMERA_1;
    assert_eq!(camera_id.to_address_byte(), 0x81);

    let _model = CameraVariant::PtzOpticsG2;
    let _socket = ViscaSocket::S1;
    let _cache = StateCache::new();
    let _limits = PanTiltLimits::new();
    let _flip = CachedFlipState {
        horizontal: false,
        vertical: false,
    };
}
