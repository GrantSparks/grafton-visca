use grafton_visca::{blocking::Operation, operation::AppliedOnly};

fn invalid(handle: Operation<AppliedOnly>) {
    let _ = handle.settled();
    //~^ E0599 "named `settled` found for struct"
}

fn main() {
    let _ = invalid;
}
