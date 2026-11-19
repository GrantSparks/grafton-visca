//! Move a PTZOptics G2 using blocking operation handles. Pass the camera
//! address on the command line or set `VISCA_CAMERA_ADDR`.

mod support;

use std::{env, thread::sleep, time::Duration};

use grafton_visca::{
    blocking::Camera, blocking::CameraSession, profiles::PtzOpticsG2, types::AffectedAxes,
    types::IdleWait, types::PanSpeed, types::PanTiltDirection, types::PanTiltSpeed,
    types::TiltSpeed, Error,
};

use support::finish_session;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = env::args()
        .nth(1)
        .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
        .ok_or("camera address required (argument or VISCA_CAMERA_ADDR)")?;

    let session = CameraSession::<PtzOpticsG2>::connect_tcp(&address)?;
    let camera = session.camera();
    let result = movement(camera);
    finish_session(result, session.close())?;
    Ok(())
}

/// Attempts a pan/tilt STOP on scope exit, including an early return or panic.
/// `Drop` cannot return a STOP error.
struct StopPanTiltOnExit<'a> {
    camera: &'a Camera<PtzOpticsG2>,
}

impl Drop for StopPanTiltOnExit<'_> {
    fn drop(&mut self) {
        if let Ok(mut stop) = self.camera.pan_tilt().stop() {
            let _ = stop.applied();
        }
    }
}

fn movement(camera: &Camera<PtzOpticsG2>) -> Result<(), Error> {
    // Home has a target position. The zoom drive continues after application.
    camera.pan_tilt().home()?.settled()?;
    camera.zoom().tele()?.applied()?;
    camera.zoom().stop()?.applied()?;

    {
        let _stop_on_exit = StopPanTiltOnExit { camera };
        let mut drive = camera.pan_tilt().drive(
            PanTiltDirection::Up,
            PanTiltSpeed::new(PanSpeed::new(6)?, TiltSpeed::new(6)?),
        )?;
        sleep(Duration::from_millis(250));
        drive.applied()?;
    }

    // Detaching leaves the zoom running until the explicit STOP.
    camera.zoom().tele()?.detach();
    sleep(Duration::from_millis(250));
    camera.zoom().stop()?.applied()?;

    camera.motion().wait_until_idle(IdleWait::new(
        AffectedAxes::PAN_TILT,
        Duration::from_secs(30),
    ))?;
    Ok(())
}
