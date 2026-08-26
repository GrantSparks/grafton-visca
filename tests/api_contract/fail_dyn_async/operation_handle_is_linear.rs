use grafton_visca::dynapi::DynTargetedOperation;

async fn use_after_terminal(handle: DynTargetedOperation) {
    let _ = handle.settled().await;
    let _ = handle.settled().await;
}

fn main() {
    let _ = use_after_terminal;
}
