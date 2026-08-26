use grafton_visca::{blocking::Operation, completion::Targeted};

fn use_after_terminal(handle: Operation<'static, Targeted>) {
    let _ = handle.settled();
    let _ = handle.settled();
}

fn main() {
    let _ = use_after_terminal;
}
