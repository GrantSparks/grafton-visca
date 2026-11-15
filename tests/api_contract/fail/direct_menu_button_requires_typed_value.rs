use grafton_visca::command::{DirectMenuControl, DirectMenuTarget};
fn main() {
    let _ = DirectMenuControl::new(DirectMenuTarget::NdFilter, 5u8);
}
//~ E0308
//~ "expected `MenuButton`, found `u8`"
