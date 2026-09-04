use std::marker::PhantomData;

use grafton_visca::{
    command::{BoolConvention, FlipState, Nibbles, Payload, Response},
    zoom_from_normalized, CameraId, StateCache, UnitInterval, ViscaSocket, ZoomDomain,
};

fn main() {
    let camera_id = CameraId::CAMERA_1;
    assert_eq!(camera_id.to_address_byte(), 0x81);

    let _socket = ViscaSocket::S1;
    // The canonical root cache is an owner-backed target view; it is not a
    // standalone mutable value.  Construction is exercised through Session
    // and Camera facade tests, while this fixture keeps the root type public.
    let _: PhantomData<StateCache> = PhantomData;
    let _flip = FlipState {
        horizontal: false,
        vertical: false,
    };
    let _position = grafton_visca::camera::PanTiltPosition::new(0, 0);
    let _response = Response::Completion { socket: None };
    let payload = Payload::new(&[0x02]);
    let _ = payload
        .parse_bool("contract", BoolConvention::OnIs02)
        .unwrap();
    let _ = Nibbles::<4>::try_from(Payload::new(&[0, 1, 2, 3])).unwrap();
    let root_value = UnitInterval::new(0.5).unwrap();
    let units_value: grafton_visca::units::UnitInterval = root_value;
    assert_eq!(units_value.value(), 0.5);
    let _ = zoom_from_normalized(root_value, ZoomDomain::Optical, 0x4000, None)
        .expect("UnitInterval is accepted by profile-aware zoom helpers");

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
