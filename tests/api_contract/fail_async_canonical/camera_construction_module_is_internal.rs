// `Connect` and the connect builders are re-exported from `camera` and the
// crate root; the module implementing them is private. This replaces the 1.x
// `camera::convenience` fixture, whose module does not exist in 2.0.

fn main() {
    let _ = core::any::TypeId::of::<grafton_visca::camera::construction::Connect>();
}

//~ E0603
//~ "module `construction` is private"
