//! Concurrent access to one Tokio owner-backed session.
//!
//! Cloned typed camera views share the same serialized owner. The example
//! performs a read and a movement concurrently, then stops the movement and
//! waits for the selected axis to become idle.

use std::{env, time::Duration};

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, IdleWait},
    runtime::TokioRuntime,
    AffectedAxes, Connect, Error,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = env::args()
        .nth(1)
        .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
        .unwrap_or_else(|| "192.168.0.110".to_owned());
    let runtime = TokioRuntime::from_current()?;
    let session = Connect::open_tcp::<PtzOpticsG2, _>(&address, runtime).await?;
    let camera = session.camera::<PtzOpticsG2>()?;

    let movement_camera = camera.clone();
    let inquiry_camera = camera.clone();
    let power = inquiry_camera.power();
    let (movement, power) = tokio::join!(
        async move {
            let operation = movement_camera.zoom().tele().await?;
            operation.applied().await
        },
        power.state(),
    );
    report("zoom movement", movement)?;
    println!("Power: {}", if power? { "on" } else { "off" });

    camera.zoom().stop().await?.applied().await?;
    camera
        .motion()
        .wait_until_idle(IdleWait::new(AffectedAxes::ZOOM, Duration::from_secs(5)))
        .await?;
    session.close().await?;
    Ok(())
}

fn report(label: &str, result: Result<(), Error>) -> Result<(), Error> {
    if let Err(error) = &result {
        eprintln!("{label}: {error}");
    }
    result
}
