fn main() {
    let _ = core::any::TypeId::of::<grafton_visca::command::zoom_wire::Zoom>();
    let _ = core::any::TypeId::of::<grafton_visca::command::preset_wire::PresetNumber>();
    let _ = core::any::TypeId::of::<grafton_visca::command::inquiry::PowerInquiry>();
}

//~ E0603
//~ "module `zoom_wire` is private"
//~ "module `preset_wire` is private"
//~ "module `inquiry` is private"
