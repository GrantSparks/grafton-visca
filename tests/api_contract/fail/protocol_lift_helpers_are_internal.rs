fn main() {
    let _ = grafton_visca::command::response::lift::lift_inquiry_for::<
        grafton_visca::profiles::GenericVisca,
    >;
}

// The path used to name `command::response::lift_inquiry_for`, which does not
// exist: the helper lives in `command::response::lift`. The fixture therefore
// reported an unresolved value (E0425) beside the privacy error it exists to
// pin, and the loose harness accepted either.

//~ E0603
//~ "module `response` is private"
