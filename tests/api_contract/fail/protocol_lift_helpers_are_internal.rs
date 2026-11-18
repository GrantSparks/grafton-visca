fn main() {
    let _ = grafton_visca::command::response::lift::lift_inquiry;
    //~^ E0603 "module `response` is private"
}

// `lift_inquiry` is the single crate-internal lifting helper; it lives in the
// private `command::response::lift` module, so naming it from outside the
// crate is a privacy error.
