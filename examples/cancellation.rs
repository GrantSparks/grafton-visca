//! Drive the zoom, request cancellation, then send a STOP on every cancellation
//! outcome. The Sony EVI-H100 supports socket cancellation over RS-232C at
//! 9,600 bps. To try a PTZOptics G2 over TCP, pass `--g2-unsupported`.
//! Supply the serial port or camera address as an argument or in `VISCA_CAMERA_ADDR`.

mod support;

use std::{env, time::Duration};

use grafton_visca::{
    operation::CancellationOutcome, profiles::PtzOpticsG2, profiles::SonyEviH100, r#async::Camera,
    r#async::CameraSession, runtime::TokioRuntime, Error,
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
        .ok_or("serial port or camera address required (argument or VISCA_CAMERA_ADDR)")?;

    let runtime = TokioRuntime::from_current()?;
    if g2_unsupported {
        let session = CameraSession::<PtzOpticsG2>::connect_tcp(&address, runtime).await?;
        let result = cancel_g2_drive(session.camera()).await;
        finish_session(result, session.close().await)?;
    } else {
        let session =
            CameraSession::<SonyEviH100>::connect_serial(&address, 9_600, runtime).await?;
        let result = cancel_supported_drive(session.camera()).await;
        finish_session(result, session.close().await)?;
    }
    Ok(())
}

async fn cancel_supported_drive(camera: &Camera<SonyEviH100>) -> Result<(), Error> {
    // Give the drive time to reach the camera before requesting cancellation.
    let mut operation = camera.zoom().tele().await?;
    tokio::time::sleep(Duration::from_millis(100)).await;

    let cancellation = operation.cancel_with_timeout(Duration::from_secs(2)).await;

    // Even a confirmed protocol cancellation leaves physical motion uncertain.
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

    // G2 can report completion while the lens is moving. If the command is
    // still in flight, this profile returns CancellationNotSupported.
    let cancellation = operation.cancel().await;

    let stop = match camera.zoom().stop().await {
        Ok(mut stop) => stop.applied().await,
        Err(error) => Err(error),
    };

    let refused = match cancellation {
        Err(Error::CancellationNotSupported) => {
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
