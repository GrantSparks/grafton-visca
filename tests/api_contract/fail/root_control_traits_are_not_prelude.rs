fn main() {
    let _ = core::any::TypeId::of::<
        dyn grafton_visca::PowerControl<Mode = grafton_visca::mode::Blocking>,
    >();
}
