fn main() {
    let _ = grafton_visca::command::response::lift::lift_inquiry;
}

// `lift_inquiry` is the single crate-internal lifting helper; it lives in the
// private `command::response::lift` module, so naming it from outside the
// crate is a privacy error.

//~ E0603
//~ "module `response` is private"
