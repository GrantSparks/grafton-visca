fn main() {
    let _ = core::any::TypeId::of::<grafton_visca::command::zoom::Zoom>();
    let _ = core::any::TypeId::of::<grafton_visca::command::preset::PresetNumber>();
    let _ = core::any::TypeId::of::<grafton_visca::command::inquiry::PowerInquiry>();
}

//~ E0603
//~ "module `zoom` is private"
//~ "module `preset` is private"
//~ "module `inquiry` is private"
