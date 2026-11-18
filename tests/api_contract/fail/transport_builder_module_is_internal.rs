fn main() {
    let _ = grafton_visca::transport::builder::IoBounds::default();
    //~^ E0603 "module `builder` is private"
}
