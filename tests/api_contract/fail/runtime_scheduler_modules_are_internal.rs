fn main() {
    let _ = core::any::TypeId::of::<grafton_visca::runtime::core::SchedulerCore>();
    let _ = core::any::TypeId::of::<dyn grafton_visca::runtime::driver::SchedulerLike>();
}
