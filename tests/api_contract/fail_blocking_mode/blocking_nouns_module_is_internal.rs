// The blocking noun accessors are re-exported from `grafton_visca::blocking`;
// the module declaring them is private. This is the blocking twin of the
// async `async_nouns_module_is_internal` fixture.

fn main() {
    use grafton_visca::blocking::blocking_nouns::PowerAccessor;

    let _ = core::any::TypeId::of::<PowerAccessor<'static, grafton_visca::profiles::PtzOpticsG2>>();
}

//~ E0603
//~ "module `blocking_nouns` is private"
