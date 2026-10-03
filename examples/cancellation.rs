//! Async cancellation on a cancel-capable profile, with an optional PTZOptics
//! G2 unsupported-post-send demonstration.
//!
//! Queued work is always locally cancellable, and on a socket-cancel-capable
//! profile a command that has already been written can be cancelled too.
//! There, `cancel_with_timeout()` concludes with `Cancelled` or `Completed`.
//! This example uses the cancel-capable `PtzOpticsG3` by default, so that
//! supported path is reachable on the hardware it names. Pass
//! `--g2-unsupported` to demonstrate the sole built-in profile without socket
//! cancellation: after a G2 command is written, `cancel()` refuses with
//! `Error::NotSupported`, and the same handle still observes the command.
//!
//! This example moves real hardware. Set `VISCA_CAMERA_ADDR` or pass an address
//! on the command line.

mod support;

use std::{env, time::Duration};

use grafton_visca::{
    camera::profiles::{PtzOpticsG2, PtzOpticsG3},
    runtime::TokioRuntime,
    Camera, CancellationOutcome, Connect, Error,
};

use support::finish_session;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut address = None;
    let mut g2_unsupported = false;
    for argument in env::args().skip(1) {
        match argument.as_str() {
            "--g2-unsupported" => g2_unsupported = true,
            value if value.starts_with('-') => {
                return Err(format!("unknown option `{value}`").into())
            }
            _ if address.is_none() => address = Some(argument),
            value => return Err(format!("unexpected argument `{value}`").into()),
        }
    }
    let address = address
        .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
        .ok_or("camera address required (argument or VISCA_CAMERA_ADDR)")?;

    let runtime = TokioRuntime::from_current()?;
    if g2_unsupported {
        let session = Connect::open_tcp::<PtzOpticsG2, _>(&address, runtime).await?;
        let result = cancel_g2_drive(session.camera()).await;
        finish_session(result, session.close().await)?;
    } else {
        let session = Connect::open_tcp::<PtzOpticsG3, _>(&address, runtime).await?;
        let result = cancel_g3_drive(session.camera()).await;
        finish_session(result, session.close().await)?;
    }
    Ok(())
}

async fn cancel_g3_drive(camera: &Camera<PtzOpticsG3>) -> Result<(), Error> {
    // Start a continuous zoom drive and let its first write reach the camera, so
    // the cancel below exercises the post-send path rather than a local queue
    // removal.
    let mut operation = camera.zoom().tele().await?;
    tokio::time::sleep(Duration::from_millis(100)).await;

    match operation
        .cancel_with_timeout(Duration::from_secs(2))
        .await?
    {
        CancellationOutcome::Cancelled => {
            println!("cancellation confirmed: the drive was cancelled");
        }
        CancellationOutcome::Completed => {
            println!("the drive completed before cancellation could win");
        }
        outcome => println!("cancellation concluded: {outcome:?}"),
    }
    // A cancellation outcome is protocol evidence, not proof that hardware is
    // physically still. Always pair a continuous drive with an explicit STOP.
    camera.zoom().stop().await?.applied().await?;

    Ok(())
}

async fn cancel_g2_drive(camera: &Camera<PtzOpticsG2>) -> Result<(), Error> {
    let mut operation = camera.zoom().tele().await?;
    tokio::time::sleep(Duration::from_millis(100)).await;

    match operation.cancel().await {
        Err(Error::NotSupported) => {}
        Err(error) => return Err(error),
        Ok(outcome) => {
            println!("the drive concluded before cancellation was refused: {outcome:?}");
            return Ok(());
        }
    }
    // The refused cancellation left the drive running and this handle
    // observing it; an applied STOP is what ends the motion.
    println!("G2 cannot cancel a sent command; applying an explicit STOP");
    camera.zoom().stop().await?.applied().await?;
    match operation.applied_with_timeout(Duration::from_secs(2)).await {
        Ok(()) => println!("the drive concluded as applied"),
        Err(error) => println!("the drive concluded with {error}"),
    }

    Ok(())
}
