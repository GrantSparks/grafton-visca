use grafton_visca::r#async::{Camera, Operation, Session};
//~^ E0432 "unresolved import `grafton_visca::r#async`"

fn main() {
    let _: Option<Camera<grafton_visca::profiles::PtzOpticsG2>> = None;
    let _: Option<Session> = None;
    let _: Option<Operation<grafton_visca::operation::AppliedOnly>> = None;
}
