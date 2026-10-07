use grafton_visca::dynapi::DynTargetedOperation;

// Detaching consumes the handle (#777): nothing can observe it afterwards.
async fn use_after_detach(mut handle: DynTargetedOperation) {
    handle.detach();
    let _ = handle.applied().await;
}

fn main() {
    let _ = use_after_detach;
}

//~ E0382
//~ "borrow of moved value: `handle`"
