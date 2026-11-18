fn main() {
    let _ = core::any::TypeId::of::<grafton_visca::command::zoom_wire::Zoom>();
    //~^ E0603 "module `zoom_wire` is private"
    let _ = core::any::TypeId::of::<grafton_visca::command::preset_wire::PresetNumber>();
    //~^ E0603 "module `preset_wire` is private"
    let _ = core::any::TypeId::of::<grafton_visca::command::inquiry::PowerInquiry>();
    //~^ E0603 "module `inquiry` is private"
}
