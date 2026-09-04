use grafton_visca::{
    profiles::{GenericVisca, SonyBRC300},
    Camera,
};

fn main() {
    let generic: Option<Camera<GenericVisca>> = None;
    let generic = generic.as_ref().unwrap();
    let _ = generic.image().freeze_on();
    let _ = generic.image().freeze_off();
    let _ = generic.image().defog_level();

    let brc300: Option<Camera<SonyBRC300>> = None;
    let brc300 = brc300.as_ref().unwrap();
    let _ = brc300.image().freeze_on();
    let _ = brc300.image().freeze_off();
    let _ = brc300.image().defog_level();
}

//~ E0277
//~ "GenericVisca` does not declare image-processing support"
//~ "profile `SonyBRC300` does not declare image-processing support"
