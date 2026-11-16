//! Each coercible noun row accepts a coarse SpeedLevel on both facades.

#[cfg(feature = "blocking")]
fn blocking_speed_inputs(
    camera: &grafton_visca::blocking::Camera,
    position: grafton_visca::types::PanTiltPosition,
) -> grafton_visca::Result<()> {
    use grafton_visca::types::{PanTiltDirection, SpeedLevel};

    camera
        .pan_tilt()
        .drive(PanTiltDirection::Up, SpeedLevel::Medium)?
        .detach();
    camera
        .pan_tilt()
        .absolute(position, SpeedLevel::Medium)?
        .detach();
    camera
        .pan_tilt()
        .relative(position, SpeedLevel::Medium)?
        .detach();
    camera.zoom().tele_variable(SpeedLevel::Medium)?.detach();
    camera.zoom().wide_variable(SpeedLevel::Medium)?.detach();
    camera.focus().far_variable(SpeedLevel::Medium)?.detach();
    camera.focus().near_variable(SpeedLevel::Medium)?.detach();
    Ok(())
}

#[cfg(feature = "async")]
async fn async_speed_inputs(
    camera: &grafton_visca::r#async::Camera,
    position: grafton_visca::types::PanTiltPosition,
) -> grafton_visca::Result<()> {
    use grafton_visca::types::{PanTiltDirection, SpeedLevel};

    camera
        .pan_tilt()
        .drive(PanTiltDirection::Up, SpeedLevel::Medium)
        .await?
        .detach();
    camera
        .pan_tilt()
        .absolute(position, SpeedLevel::Medium)
        .await?
        .detach();
    camera
        .pan_tilt()
        .relative(position, SpeedLevel::Medium)
        .await?
        .detach();
    camera
        .zoom()
        .tele_variable(SpeedLevel::Medium)
        .await?
        .detach();
    camera
        .zoom()
        .wide_variable(SpeedLevel::Medium)
        .await?
        .detach();
    camera
        .focus()
        .far_variable(SpeedLevel::Medium)
        .await?
        .detach();
    camera
        .focus()
        .near_variable(SpeedLevel::Medium)
        .await?
        .detach();
    Ok(())
}

fn main() {}
