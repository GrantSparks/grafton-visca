use grafton_visca::dynapi::DynAppliedOperation;

async fn invalid(handle: DynAppliedOperation) {
    let _ = handle.settled().await;
}

fn main() {}
