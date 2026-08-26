use grafton_visca::{completion::Targeted, Operation};

async fn use_after_terminal(handle: Operation<Targeted>) {
    let _ = handle.settled().await;
    let _ = handle.settled().await;
}

fn main() {
    let _ = use_after_terminal;
}
