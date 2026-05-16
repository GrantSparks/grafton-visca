fn main() {
    use grafton_visca::camera::accessors::PowerAccessor;

    let _ = core::any::TypeId::of::<PowerAccessor<'static, (), (), (), ()>>();
}
