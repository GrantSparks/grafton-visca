use grafton_visca::{blocking::Operation, completion::Targeted};

// Waits borrow the handle exclusively (#777): a shared reference cannot wait,
// so one handle is never observed by two waits at once.
fn shared_wait(handle: &Operation<Targeted>) {
    let _ = handle.settled();
}

fn main() {
    let _ = shared_wait;
}

//~ E0596
//~ "cannot borrow `*handle` as mutable, as it is behind a `&` reference"
