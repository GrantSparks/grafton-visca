use grafton_visca::{completion::AppliedOnly, Operation};

async fn invalid(handle: Operation<AppliedOnly>) {
    let _ = handle.settled().await;
}

fn main() {}

//~ E0599
//~ "named `settled` found for struct"
