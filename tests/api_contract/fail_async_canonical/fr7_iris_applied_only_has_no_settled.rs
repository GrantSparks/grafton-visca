use grafton_visca::{profiles::SonyFr7, r#async::Camera, types::IrisStep};

async fn invalid(camera: &Camera<SonyFr7>) {
    let mut iris = camera.exposure().iris_up_by(IrisStep::MIN).await.unwrap();
    let _ = iris.settled().await;
}

fn main() {}

//~ E0599
//~ "named `settled` found for struct"
