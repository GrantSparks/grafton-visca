use grafton_visca::{blocking::Operation, operation::Targeted};

// Detaching consumes the handle (#777): nothing can observe it afterwards.
fn use_after_detach(mut handle: Operation<Targeted>) {
    handle.detach();
    let _ = handle.applied();
    //~^ E0382 "borrow of moved value: `handle`"
}

fn main() {
    let _ = use_after_detach;
}
