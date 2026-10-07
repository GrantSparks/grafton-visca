use grafton_visca::{completion::Targeted, Operation};

// Detaching consumes the handle (#777): nothing can observe it afterwards.
async fn use_after_detach(mut handle: Operation<Targeted>) {
    handle.detach();
    let _ = handle.applied().await;
}

fn main() {
    let _ = use_after_detach;
}

//~ E0382
//~ "borrow of moved value: `handle`"
