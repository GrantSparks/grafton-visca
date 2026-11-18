fn main() {
    let _ = grafton_visca::units::Normalized::new(0.5);
    //~^ E0433 "cannot find `Normalized` in `units`"
    //~^^ E0603 "module `units` is private"
}
