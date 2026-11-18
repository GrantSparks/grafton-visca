use grafton_visca::{operation::AppliedOnly, r#async::Operation};

async fn invalid(handle: Operation<AppliedOnly>) {
    let _ = handle.settled().await;
    //~^ E0599 "named `settled` found for struct"
}

fn main() {}
