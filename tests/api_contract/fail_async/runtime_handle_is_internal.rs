fn main() {
    let _ = core::any::TypeId::of::<
        grafton_visca::runtime::RuntimeHandle<
            grafton_visca::profiles::PtzOpticsG2,
            grafton_visca::TokioExecutor,
        >,
    >();
}
