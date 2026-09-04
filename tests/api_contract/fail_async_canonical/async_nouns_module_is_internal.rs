// The async noun accessors are re-exported from the crate root; the module
// declaring them is private. This replaces the 1.x
// `camera::accessors` fixture, whose module does not exist in 2.0.

fn main() {
    use grafton_visca::async_nouns::PowerAccessor;

    let _ = core::any::TypeId::of::<PowerAccessor<'static, grafton_visca::profiles::PtzOpticsG2>>();
}

//~ E0603
//~ "module `async_nouns` is private"
