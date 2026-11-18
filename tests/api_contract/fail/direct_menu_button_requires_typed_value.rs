use grafton_visca::{command::menu::Direct, types::DirectMenuTarget};
fn main() {
    let _ = Direct::new(DirectMenuTarget::NdFilter, 5u8);
    //~^ E0308 "mismatched types"
}
