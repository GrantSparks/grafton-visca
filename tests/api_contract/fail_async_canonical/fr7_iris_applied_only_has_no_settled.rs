use grafton_visca::{profiles::SonyFR7, types::IrisStep, Camera};

async fn invalid(camera: &Camera<SonyFR7>) {
    let mut iris = camera.exposure().iris_up_by(IrisStep::MIN).await.unwrap();
    let _ = iris.settled().await;
}

fn main() {}

//~ E0599
//~ "named `settled` found for struct"
