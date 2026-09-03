//! Concurrent access to one Tokio owner-backed session.
//!
//! Cloned typed camera views share the same serialized owner. The example
//! performs a read and a movement concurrently, then stops the movement and
//! waits for the selected axis to become idle.
//!
//! The concurrent region runs inside `concurrent_work`, which stops the zoom on
//! both of its return paths and closes the session through `finish_session`, so
//! an early `?` neither leaks motion nor leaks the session.

mod support;

use std::{env, time::Duration};

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, IdleWait},
    runtime::TokioRuntime,
    AffectedAxes, Camera, Connect, Error,
};

use support::finish_session;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = env::args()
        .nth(1)
        .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
        .unwrap_or_else(|| "192.168.0.110".to_owned());
    let runtime = TokioRuntime::from_current()?;
    let session = Connect::open_tcp::<PtzOpticsG2, _>(&address, runtime).await?;
    let result = concurrent_work(session.camera()).await;
    finish_session(result, session.close().await)?;
    Ok(())
}

/// Runs the concurrent region, then stops the zoom on every return path.
///
/// `Drop` cannot await, so the async form of a scoped stop is this wrapper:
/// run the fallible body, stop, then report the body's failure ahead of the
/// stop's. It covers the body's `Ok` and `Err` returns. A panic inside the
/// body, or a caller dropping this future before it completes, would not reach
/// the stop — a synchronous `Drop` guard is what covers those.
async fn concurrent_work(camera: &Camera<PtzOpticsG2>) -> Result<(), Error> {
    let result = read_and_move(camera).await;
    let stopped = stop_zoom(camera).await;
    result.and(stopped)
}

async fn read_and_move(camera: &Camera<PtzOpticsG2>) -> Result<(), Error> {
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
    Ok(())
}

async fn stop_zoom(camera: &Camera<PtzOpticsG2>) -> Result<(), Error> {
    camera.zoom().stop().await?.applied().await?;
    camera
        .motion()
        .wait_until_idle(IdleWait::new(AffectedAxes::ZOOM, Duration::from_secs(5)))
        .await
}

fn report(label: &str, result: Result<(), Error>) -> Result<(), Error> {
    if let Err(error) = &result {
        eprintln!("{label}: {error}");
    }
    result
}
