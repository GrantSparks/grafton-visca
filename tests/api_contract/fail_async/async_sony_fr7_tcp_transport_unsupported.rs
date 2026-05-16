use grafton_visca::{camera::Connect, profiles::SonyFR7, runtime::TokioRuntime};

async fn unsupported_tcp(runtime: TokioRuntime) {
    let _ = Connect::open_tcp_async::<SonyFR7, _>("127.0.0.1", runtime).await;
}

fn main() {
    let _ = unsupported_tcp;
}
