// `camera::movement` re-exports its motion value types through `camera`; the
// module itself stays private so the layout can move without a breaking
// change. This replaces the 1.x `camera::builder` fixture, whose module does
// not exist in 2.0 and which therefore pinned nothing.

fn main() {
    let _ = core::any::TypeId::of::<grafton_visca::camera::movement::PanTiltPosition>();
}

//~ E0603
//~ "module `movement` is private"
