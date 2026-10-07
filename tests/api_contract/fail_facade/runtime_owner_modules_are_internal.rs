// The protocol engine and the owner projections behind it are crate-private.
// This replaces the 1.x `runtime::core` / `runtime::driver` fixture, whose
// modules do not exist in 2.0: it failed with "cannot find", not "is private",
// so it pinned nothing about the modules 2.0 actually keeps internal.

fn main() {
    let _ = core::any::TypeId::of::<grafton_visca::runtime::engine::ProtocolEngine>();
    let _ = core::any::TypeId::of::<grafton_visca::runtime::owner::OwnerPolicy>();
}

//~ E0603
//~ "module `engine` is private"
//~ "module `owner` is private"
