use grafton_visca::{blocking::Operation, completion::Targeted};

// Detaching consumes the handle (#777): nothing can observe it afterwards.
fn use_after_detach(mut handle: Operation<'static, Targeted>) {
    handle.detach();
    let _ = handle.applied();
}

fn main() {
    let _ = use_after_detach;
}

//~ E0382
//~ "borrow of moved value: `handle`"
