//! Async cancellation: the supported outcome and the PTZOptics G2
//! unsupported-post-send `NotSupported` case.
//!
//! Queued work is always locally cancellable, and on a socket-cancel-capable
//! profile a command that has already been written can be cancelled too — there
//! `cancel()` returns a `Cancellation` whose `outcome()` is `Cancelled` or
//! `Completed`. The built-in `PtzOpticsG2` is the one profile *without*
//! socket-cancel, so once a command has been written `cancel()` refuses with
//! `Error::NotSupported` and hands the handle back inside `CancelRejected`. The
//! original operation is still live, so the way to end motion on that profile is
//! an explicit STOP.
//!
//! This example moves real hardware. Set `VISCA_CAMERA_ADDR` or pass an address
//! on the command line.

mod support;

use std::{env, time::Duration};

use grafton_visca::{
    camera::profiles::PtzOpticsG2, runtime::TokioRuntime, CancellationOutcome, Connect, Error,
    Session,
};

use support::finish_session;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = env::args()
        .nth(1)
        .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
        .ok_or("camera address required (argument or VISCA_CAMERA_ADDR)")?;

    let runtime = TokioRuntime::from_current()?;
    let session = Connect::open_tcp::<PtzOpticsG2, _>(&address, runtime).await?;
    let result = cancel_a_drive(&session).await;
    finish_session(result, session.close().await)?;
    Ok(())
}

async fn cancel_a_drive(session: &Session) -> Result<(), Error> {
    let camera = session.camera::<PtzOpticsG2>()?;

    // Start a continuous zoom drive and let its first write reach the camera, so
    // the cancel below exercises the post-send path rather than a local queue
    // removal.
    let operation = camera.zoom().tele().await?;
    tokio::time::sleep(Duration::from_millis(100)).await;

    match operation.cancel().await {
        // Supported outcome: a socket-cancel-capable profile (or a still-queued
        // request on any profile) yields a `Cancellation` token. Observe it with
        // a bounded deadline. The outcome proves the protocol result, not that
        // motion physically stopped, so a confirmed cancel is still paired with
        // an explicit STOP.
        Ok(cancellation) => match cancellation.outcome(Duration::from_secs(2)).await? {
            CancellationOutcome::Cancelled => {
                println!("cancellation confirmed: the drive was cancelled");
                camera.zoom().stop().await?.applied().await?;
            }
            CancellationOutcome::Completed => {
                println!("the drive completed before cancellation could win");
            }
        },
        // PtzOpticsG2 post-send: no socket-cancel, so `cancel()` refuses with
        // `NotSupported` and returns the handle inside `CancelRejected`. The
        // original operation is untouched — recover the handle, STOP, then detach
        // the recovered handle (drop is detach; it never stops hardware).
        Err(rejected) if matches!(rejected.error(), Error::NotSupported) => {
            println!("this profile cannot cancel a sent command; stopping instead");
            match rejected.into_operation() {
                Some(operation) => {
                    camera.zoom().stop().await?.applied().await?;
                    operation.detach();
                }
                None => return Err(Error::RuntimeShutdown),
            }
        }
        // Any other refusal is a real error; `into_error` keeps the reason and is
        // exactly what the `From<CancelRejected>` conversion behind `?` would do.
        Err(rejected) => return Err(rejected.into_error()),
    }

    Ok(())
}
