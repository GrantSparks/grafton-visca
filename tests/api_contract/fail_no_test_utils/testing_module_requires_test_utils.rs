fn main() {
    let _ = core::any::TypeId::of::<grafton_visca::testing::testkit::ManualClockExecutor>();
    //~^ E0433 "cannot find `testing` in `grafton_visca`"
}
