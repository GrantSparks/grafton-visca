use grafton_visca::{Camera, Operation, Session};

fn main() {
    let _: Option<Camera<grafton_visca::profiles::PtzOpticsG2>> = None;
    let _: Option<Session> = None;
    let _: Option<Operation<grafton_visca::completion::AppliedOnly>> = None;
}

//~ E0432
//~ "unresolved imports `grafton_visca::Camera`"
