//! Tokio operation handles with exact per-command completion semantics.
//!
//! This example moves real hardware. It snapshots the current pose, demonstrates
//! pan/tilt and zoom operation handles, and restores the snapshot.
//!
//! Run with:
//! ```sh
//! cargo run --example operation_handles_async --features runtime-tokio -- <camera-address>
//! ```

mod support;

#[path = "support/operation.rs"]
mod operation_support;

use std::{env, io, time::Duration};

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, Connect},
    command::{PanTilt, PanTiltDirection, Zoom},
    runtime::TokioRuntime,
    types::{PanPosition, TiltPosition},
    Error, SpeedLevel,
};

use operation_support::{finish_cleanup, record_cleanup_error};
use support::finish_session;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = camera_address()?;
    let _ = tracing_subscriber::fmt::try_init();

    let runtime = TokioRuntime::from_current()?;
    let camera = Connect::open_tcp_async::<PtzOpticsG2, _>(&address, runtime).await?;

    let original_pose = match camera.pan_tilt().position().await {
        Ok(pan_tilt) => camera.zoom().position().await.map(|zoom| (pan_tilt, zoom)),
        Err(error) => Err(error),
    };

    let operation_result: Result<(), Error> = match original_pose {
        Ok((original_pan_tilt, original_zoom)) => {
            let restore_pan_tilt = PanTilt::AbsolutePosition {
                pan: PanPosition::new(original_pan_tilt.pan)?,
                tilt: TiltPosition::new(original_pan_tilt.tilt)?,
                pan_speed: SpeedLevel::Medium.into(),
                tilt_speed: SpeedLevel::Medium.into(),
            };
            let pan_tilt_stop = PanTilt::Move {
                direction: PanTiltDirection::Stop,
                pan_speed: SpeedLevel::Slow.into(),
                tilt_speed: SpeedLevel::Slow.into(),
            };

            let movement_result = async {
                // A targeted move has a meaningful settled state. One
                // deadline covers protocol completion and physical motion.
                camera
                    .submit(&PanTilt::Home)
                    .await?
                    .await_settled(Duration::from_secs(20))
                    .await?;

                // Async submission returns after scheduler acceptance. Detach
                // leaves the command scheduler-owned, so always follow with STOP.
                camera.submit(&Zoom::TeleStd).await?.detach();
                tokio::time::sleep(Duration::from_millis(250)).await;
                camera
                    .submit(&Zoom::Stop)
                    .await?
                    .await_applied(Duration::from_secs(2))
                    .await?;
                Ok(())
            }
            .await;

            // Cleanup is attempted even when the operation fails. Keep going
            // after a failed STOP so the position restore still gets a chance.
            let mut cleanup_error = None;
            let stop_result = match camera.submit(&Zoom::Stop).await {
                Ok(handle) => handle.await_applied(Duration::from_secs(2)).await,
                Err(error) => Err(error),
            };
            record_cleanup_error(&mut cleanup_error, stop_result, "stopping zoom");

            let pan_tilt_stop_result = match camera.submit(&pan_tilt_stop).await {
                Ok(handle) => handle.await_applied(Duration::from_secs(2)).await,
                Err(error) => Err(error),
            };
            record_cleanup_error(
                &mut cleanup_error,
                pan_tilt_stop_result,
                "stopping pan/tilt",
            );

            let pan_tilt_restore_result = match camera.submit(&restore_pan_tilt).await {
                Ok(handle) => handle.await_settled(Duration::from_secs(20)).await,
                Err(error) => Err(error),
            };
            record_cleanup_error(
                &mut cleanup_error,
                pan_tilt_restore_result,
                "restoring pan/tilt",
            );

            let restore_result = match camera.submit(&Zoom::Position(original_zoom)).await {
                Ok(handle) => handle.await_settled(Duration::from_secs(20)).await,
                Err(error) => Err(error),
            };
            record_cleanup_error(&mut cleanup_error, restore_result, "restoring zoom");

            let verify_result = match camera.pan_tilt().position().await {
                Ok(restored_pan_tilt) => camera.zoom().position().await.and_then(|restored_zoom| {
                    if restored_pan_tilt == original_pan_tilt && restored_zoom == original_zoom {
                        return Ok(());
                    }

                    Err(Error::InvalidState(
                        format!(
                            "pose restoration mismatch: expected pan={} tilt={} zoom={}, got pan={} tilt={} zoom={}",
                            original_pan_tilt.pan,
                            original_pan_tilt.tilt,
                            original_zoom,
                            restored_pan_tilt.pan,
                            restored_pan_tilt.tilt,
                            restored_zoom,
                        )
                        .into(),
                    ))
                }),
                Err(error) => Err(error),
            };
            record_cleanup_error(&mut cleanup_error, verify_result, "verifying restored pose");

            finish_cleanup(movement_result, cleanup_error.map_or(Ok(()), Err))
        }
        Err(error) => Err(error),
    };

    let close_result = camera.close().await;
    finish_session(operation_result, close_result)?;
    Ok(())
}

fn camera_address() -> Result<String, io::Error> {
    let mut values = env::args().skip(1);
    let address = match values.next() {
        Some(value) if value.starts_with('-') => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown option `{value}`"),
            ));
        }
        Some(value) => value,
        None => env::var("VISCA_CAMERA_ADDR").map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "camera address required\nusage: cargo run --example operation_handles_async --features runtime-tokio -- <camera-address>\n       or set VISCA_CAMERA_ADDR",
            )
        })?,
    };

    if let Some(extra) = values.next() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "unexpected extra argument `{extra}`\nusage: cargo run --example operation_handles_async --features runtime-tokio -- [address]"
            ),
        ));
    }

    Ok(address)
}
