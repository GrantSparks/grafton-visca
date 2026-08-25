//! Blocking operation handles with exact per-command completion semantics.
//!
//! This example moves real hardware. It snapshots the current pose, demonstrates
//! pan/tilt and zoom operation handles, and restores the snapshot.
//!
//! Run with:
//! ```sh
//! cargo run --example operation_handles -- <camera-address>
//! ```

#[cfg(not(feature = "mode-async"))]
mod support;

#[cfg(not(feature = "mode-async"))]
#[path = "support/operation.rs"]
mod operation_support;

#[cfg(not(feature = "mode-async"))]
mod blocking {
    use std::{env, io, thread::sleep, time::Duration};

    use grafton_visca::{
        camera::{profiles::PtzOpticsG2, Connect},
        command::{PanTilt, PanTiltDirection, Zoom},
        types::{PanPosition, TiltPosition},
        Error, SpeedLevel,
    };

    use super::{
        operation_support::{finish_cleanup, record_cleanup_error},
        support::finish_session,
    };

    pub fn main() -> Result<(), Box<dyn std::error::Error>> {
        let address = camera_address()?;
        let _ = tracing_subscriber::fmt::try_init();

        let camera = Connect::open_tcp_blocking::<PtzOpticsG2>(&address)?;

        let original_pose = camera
            .pan_tilt()
            .position()
            .and_then(|pan_tilt| camera.zoom().position().map(|zoom| (pan_tilt, zoom)));

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

                let movement_result = (|| {
                    // A targeted move has a meaningful settled state. One
                    // deadline covers protocol completion and physical motion.
                    camera
                        .submit(&PanTilt::Home)?
                        .await_settled(Duration::from_secs(20))?;

                    // Detach is explicit fire-and-forget. It does not cancel or
                    // stop the command, so always follow it with a bounded STOP.
                    camera.submit(&Zoom::TeleStd)?.detach();
                    sleep(Duration::from_millis(250));
                    camera
                        .submit(&Zoom::Stop)?
                        .await_applied(Duration::from_secs(2))?;
                    Ok(())
                })();

                // Cleanup is attempted even when the operation fails. Keep going
                // after a failed STOP so the position restore still gets a chance.
                let mut cleanup_error = None;
                record_cleanup_error(
                    &mut cleanup_error,
                    camera
                        .submit(&Zoom::Stop)
                        .and_then(|handle| handle.await_applied(Duration::from_secs(2))),
                    "stopping zoom",
                );
                record_cleanup_error(
                    &mut cleanup_error,
                    camera
                        .submit(&pan_tilt_stop)
                        .and_then(|handle| handle.await_applied(Duration::from_secs(2))),
                    "stopping pan/tilt",
                );
                record_cleanup_error(
                    &mut cleanup_error,
                    camera
                        .submit(&restore_pan_tilt)
                        .and_then(|handle| handle.await_settled(Duration::from_secs(20))),
                    "restoring pan/tilt",
                );
                record_cleanup_error(
                    &mut cleanup_error,
                    camera
                        .submit(&Zoom::Position(original_zoom))
                        .and_then(|handle| handle.await_settled(Duration::from_secs(20))),
                    "restoring zoom",
                );
                record_cleanup_error(
                    &mut cleanup_error,
                    camera.pan_tilt().position().and_then(|restored_pan_tilt| {
                        camera.zoom().position().and_then(|restored_zoom| {
                            if restored_pan_tilt == original_pan_tilt
                                && restored_zoom == original_zoom
                            {
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
                        })
                    }),
                    "verifying restored pose",
                );

                finish_cleanup(movement_result, cleanup_error.map_or(Ok(()), Err))
            }
            Err(error) => Err(error),
        };

        let close_result = camera.close();
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
                    "camera address required\nusage: cargo run --example operation_handles -- <camera-address>\n       or set VISCA_CAMERA_ADDR",
                )
            })?,
        };

        if let Some(extra) = values.next() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "unexpected extra argument `{extra}`\nusage: cargo run --example operation_handles -- [address]"
                ),
            ));
        }

        Ok(address)
    }
}

#[cfg(not(feature = "mode-async"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    blocking::main()
}

#[cfg(feature = "mode-async")]
fn main() {
    eprintln!("This example uses the blocking API. Run without async features:");
    eprintln!("  cargo run --example operation_handles -- <camera-address>");
}
