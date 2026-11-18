// The blocking TCP/UDP transports are reached through the inherent facade
// connection constructors; the module implementing them is private.
//
// This fixture used to live under `tests/camera_first_contract/fail`, a
// directory no test binary referenced, so it never ran.

fn main() {
    let _ = core::any::TypeId::of::<grafton_visca::transport::blocking::Tcp>();
    //~^ E0603 "module `blocking` is private"
}
