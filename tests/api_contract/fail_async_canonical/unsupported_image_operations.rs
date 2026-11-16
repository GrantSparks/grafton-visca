use grafton_visca::{profiles::GenericVisca, profiles::SonyBrc300, r#async::Camera};

fn main() {
    let generic: Option<Camera<GenericVisca>> = None;
    let generic = generic.as_ref().unwrap();
    let _ = generic.image().set_freeze(true);
    let _ = generic.image().set_freeze(false);

    let brc300: Option<Camera<SonyBrc300>> = None;
    let brc300 = brc300.as_ref().unwrap();
    let _ = brc300.image().set_freeze(false);
    let _ = brc300.image().set_freeze(false);
}

//~ E0277
//~ E0599
//~ "camera profile `grafton_visca::profiles::GenericVisca` does not support `Image`"
//~ "camera profile `grafton_visca::profiles::SonyBrc300` does not support `grafton_visca::capabilities::gate::ImageFreeze`"
//~ "the method `set_freeze` exists"
