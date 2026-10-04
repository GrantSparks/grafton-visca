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
//! On real G2 hardware a continuous zoom drive usually reports ACK and
//! completion together about 50 ms after the write while the lens keeps
//! moving, so `cancel()` typically returns `Ok(Completed)` rather than the
//! refusal. Either way the drive is still running, so the G2 path applies an
//! explicit zoom STOP unconditionally after the cancellation attempt, whatever
//! `cancel()` returned. The G3 path does the same.
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

    let cancellation = operation.cancel_with_timeout(Duration::from_secs(2)).await;

    // A cancellation outcome is protocol evidence, not proof that hardware is
    // physically still. The explicit STOP runs on every outcome, including an
    // error, before any of them is reported.
    let stop = match camera.zoom().stop().await {
        Ok(mut stop) => stop.applied().await,
        Err(error) => Err(error),
    };
    match cancellation {
        Ok(CancellationOutcome::Cancelled) => {
            println!("cancellation confirmed: the drive was cancelled");
        }
        Ok(CancellationOutcome::Completed) => {
            println!("the drive completed before cancellation could win");
        }
        Ok(outcome) => println!("cancellation concluded: {outcome:?}"),
        Err(error) => {
            if let Err(stop_error) = &stop {
                eprintln!("the explicit STOP also failed: {stop_error}");
            }
            return Err(error);
        }
    }
    stop?;
    println!("explicit STOP applied");

    Ok(())
}

async fn cancel_g2_drive(camera: &Camera<PtzOpticsG2>) -> Result<(), Error> {
    let mut operation = camera.zoom().tele().await?;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // On real G2 hardware the continuous drive usually reports ACK and
    // completion together, about 50 ms after the write, while the zoom keeps
    // moving. `cancel()` then answers `Ok(Completed)` instead of refusing, so
    // neither outcome says anything about the lens.
    let cancellation = operation.cancel().await;

    // The explicit STOP is unconditional: it runs on every `cancel()` outcome,
    // including `Completed` and an unexpected error, before any of them is
    // reported. A cancellation outcome is protocol evidence, not proof that
    // hardware is physically still.
    let stop = match camera.zoom().stop().await {
        Ok(mut stop) => stop.applied().await,
        Err(error) => Err(error),
    };

    let refused = match cancellation {
        Err(Error::NotSupported) => {
            println!("G2 cannot cancel a sent command");
            true
        }
        Err(error) => {
            // Surface the STOP failure on stderr; the cancel error is returned.
            if let Err(stop_error) = &stop {
                eprintln!("the explicit STOP also failed: {stop_error}");
            }
            return Err(error);
        }
        Ok(outcome) => {
            println!("the drive concluded before cancellation was refused: {outcome:?}");
            false
        }
    };
    stop?;
    println!("explicit STOP applied");
    if refused {
        // The refused cancellation left this handle observing the drive.
        match operation.applied_with_timeout(Duration::from_secs(2)).await {
            Ok(()) => println!("the drive concluded as applied"),
            Err(error) => println!("the drive concluded with {error}"),
        }
    }

    Ok(())
}
