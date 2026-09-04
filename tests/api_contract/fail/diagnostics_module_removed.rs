fn main() {
    let _ = core::any::TypeId::of::<dyn grafton_visca::diagnostics::Diagnostics>();
}

//~ E0433
//~ "cannot find `diagnostics` in `grafton_visca`"
