use grafton_visca::{blocking::Operation, completion::AppliedOnly};

fn invalid(handle: Operation<'static, AppliedOnly>) {
    let _ = handle.settled();
}

fn main() {
    let _ = invalid;
}
